# streaming-event-notifications Specification

## Purpose
Eliminate the busy-wait polling loop in `agent_observe` by replacing it with an event-driven internal subscription model, retaining the tool-based polling as a fallback for thin clients.

## ADDED Requirements

### Requirement: TaskActor broadcasts lifecycle events internally
The system SHALL maintain a `tokio::sync::watch` channel inside the `TaskActor` keyed by `agent_id`. Drainers and completers signal the channel whenever new transcript events or status transitions occur.

#### Scenario: Observer blocked awaiting events
- **WHEN** an MCP client calls `agent_observe` with `until: "now"` on a running task
- **THEN** the server suspends the response until the watch channel fires or an external timeout elapses, instead of polling the filesystem every 50 ms.

#### Scenario: Watch channel fires on new transcript line
- **WHEN** a provider child writes a line to stdout or stderr
- **THEN** the IO drainer flushes the transcript and signals the watch channel.
- **AND** any suspended `agent_observe` resumes and returns fresh events.

### Requirement: Sentinel signal guarantees finality
The system SHALL unconditionally signal the watch channel after the child process exits and after the completion routine finishes.

#### Scenario: Task completes while observer is waiting
- **WHEN** a provider child exits
- **THEN** the watch channel receives a sentinel update.
- **AND** suspended observers perform a final read and return the terminal state.

### Requirement: Fallback poller coexists for thin clients
The system SHALL retain the synchronous `agent_observe` tool path so clients that do not support server-initiated notifications can still poll.

#### Scenario: Thin client observes without subscriptions
- **WHEN** a basic MCP client sends `agent_observe` without establishing a notification subscription
- **THEN** the server falls back to the traditional request-response cycle, returning the current state and any buffered events.

### Requirement: Backpressure protection on slow observers
The system SHALL drop watch receivers for disconnected clients and cap the number of concurrent suspended observations per task.

#### Scenario: Slow client disconnects
- **WHEN** a client drops the TCP connection or ceases responding during a suspended observe
- **THEN** the server detects the drop and releases the watch receiver, preventing memory growth.
