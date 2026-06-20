# Business Context

**Last verified:** 2026-06-16
**Sources:** Project description and codebase analysis (no user-provided business context beyond the initial description)

## Domain Overview

Agent Bridge MCP operates in the **developer tooling / agent orchestration** domain. It serves a single customer: a primary coding agent that occasionally needs to offload bounded work to specialist local agents. Rather than replacing the primary agent, Agent Bridge acts as a neutral broker — it routes one provider turn, captures bounded evidence, and insists that the caller retain responsibility for verification.

## Domain Glossary

| Term | Definition | Code Representation | Source |
|------|------------|---------------------|--------|
| Agent | Synonym for a delegated task record produced by a routed provider turn | `TaskRecord` with `agentId` | Code |
| Bridge | Launch profile that wraps the provider with safety affordances (reductions, caps, timeouts) | `LaunchProfile::Bridge` | Code |
| Bare | Launch profile that invokes the provider with minimal wrapping | `LaunchProfile::Bare` | Code |
| Unblocked | Explicit launch profile that adds provider-specific permission-bypass flags while keeping Agent Bridge workspace validation authoritative | `LaunchProfile::Unblocked` | Code |
| Caller | The upstream ACP client or MCP-adapter client that sends JSON-RPC requests | Implied `stdin` consumer | Code |
| Doctor smoke | CLI readiness and setup diagnostics | `--doctor-smoke` | Code |
| Dry run | Internal launch preview that computes the command without executing | `dry_run: true` in `TaskPreviewInput` | Code |
| Failure category | Typed taxonomy for why a provider failed | `FailureCategory` enum | Code |
| Host runner | Sidecar process owning a PTY for Claude interactive mode | `claude-host-runner` subcommand | Code |
| Isolation | Strategy for separating provider mutations from the caller's working directory | `Isolation::None` / `Worktree` | Code |
| Mode | Kind of work requested from the provider | `TaskMode`: `research`, `review`, `implement`, `command` | Code |
| Observation | Internal retrieval of transcript events and progress | `TaskManagerHandle::observe` | Code |
| Presentation actions | Capability vocabulary describing what a caller can do next | `presentation_actions()` in `provider.rs` | Code |
| Provider | A local CLI agent capable of accepting delegated work | `ProviderKind`: `claude`, `cursor`, `kimi`, `codex`, `forge`, `antigravity` | Code |
| Review packet | Summarized evidence (status, diff, changed files, recommendations) returned by the result reader | `review_packet()` in `review.rs` | Code |
| Stall risk | Heuristic computed from elapsed time and provider output cadence | `stall_risk` field in progress JSON | Code |
| Transcript | Append-only JSON Lines log of provider stdout, stderr, and lifecycle events | `transcript.jsonl` per task | Code |
| Worktree | Disposable git worktree created for isolated implementation tasks | Created via `create_worktree()` in `spawn.rs` | Code |

## User Personas

This system is designed for a **single operator** — the human configuring Agent
Bridge and the primary AI agent invoking it. There are no differentiated user
roles, RBAC policies, or multi-persona permission matrices. Security boundaries
are environmental (filesystem permissions, workspace roots, process sandboxing)
rather than role-based.

### Operator (Human)

- **Who they are:** Software engineer or power user who configures MCP clients and installs provider CLIs.
- **Responsibilities:** Set `AGENT_BRIDGE_WORKSPACES`, start the Claude host runner if needed, interpret `--doctor-smoke` output, and verify provider results before committing changes.

### Primary Agent (LLM)

- **Who they are:** The upstream coding agent consuming the ACP router or MCP adapter over stdio.
- **Capabilities:** Run one routed provider turn and fetch bounded evidence. Verification and cleanup remain caller-owned.

## Business Rules

### Delegation Rules

| Rule | Description | Verification | Enforced In | Notes |
|------|-------------|-------------|-------------|-------|
| Workspace containment | Tasks may only run inside declared workspace roots | Verified | `spawn.rs` (`safe_cwd`) | Rejects `..` segments and paths outside `AGENT_BRIDGE_WORKSPACES` |
| Prompt length cap | Prompts cannot exceed configured route/input limits | Verified | `router_runtime.rs` / task input validation | Protects against accidental paste bombs |
| Timeout clamp | Effective timeout clamps to [1s, 1800s] | Verified | `domain.rs` (`TimeoutSeconds`) | Prevents absurd budgets |
| Unknown argument rejection | Public adapter inputs reject extra fields | Verified | `mcp_adapter.rs` (`deny_unknown_fields`) | Defensive against hallucinated parameters |
| Max concurrent tasks | Ceiling of 16 active tasks unless overridden | Verified | `task.rs` (`DEFAULT_MAX_ACTIVE_TASKS`) | Prevents resource exhaustion |

### Provider Rules

| Rule | Description | Verification | Enforced In | Notes |
|------|-------------|-------------|-------------|-------|
| Mode validation | Only provider-supported modes may be requested | Verified | `provider.rs` capabilities map | E.g., Cursor lacks `command` mode |
| Profile validation | Only advertised profiles may be specified | Verified | `provider.rs` capabilities map | `bridge` / `bare` / provider-supported `unblocked` |
| Claude host runner requirement | Claude cannot launch without a reachable host socket | Verified | `spawn.rs` (`launch_task`) | Falls back to error if socket missing |
| Denial detection | Certain stderr patterns trigger immediate termination | Verified | `supervision.rs` (`drain_log` loop polls stderr) | Primarily for Codex sandbox denials |
| Output acceptability | Some providers require parseable stdout | Verified | `complete.rs` (`classify_success_exit`) | E.g., Claude parser validates JSON shapes |

### Lifecycle Rules

| Rule | Description | Verification | Enforced In | Notes |
|------|-------------|-------------|-------------|-------|
| Valid status transitions | Only permitted edges are legal | Verified | `review.rs` (`transition_status`) | Illegal transitions return error |
| Resume unsupported | Crashed-server orphans become `FailedStale` | Verified | `task.rs` startup reconciliation | Automatic worktree reclamation attempted |
| Inspection-before-cleanup | Managed cleanup is recommended only after result inspection | Verified | `review.rs` (`next_actions`) | Recommendations, not hard blocks |
| Retry for transients | Retries allowed only on `is_transient()` categories | Verified | `domain.rs` (`FailureCategory::is_transient()`) | Rate limits, timeouts, disconnects |

## Key Workflows

### Delegate a Review Turn

**Actors:** Primary Agent (LLM), Agent Bridge MCP, Provider CLI (e.g., Codex)
**Trigger:** Caller sends ACP `session/prompt` or adapter `agent_delegate` with `mode: "review"`

```mermaid
sequenceDiagram
    participant Caller as Primary Agent
    participant AB as Agent Bridge MCP
    participant PM as Provider CLI
    participant FS as Filesystem/Git

    Caller->>AB: session/prompt or tools/call agent_delegate
    AB->>FS: validate cwd inside workspace
    alt invalid cwd
        AB-->>Caller: error (outside workspaces)
    end
    AB->>PM: fork/exec provider process
    AB-->>Caller: terminal routerResult + evidenceRef
    PM->>FS: writes files / produces diff
    PM-->>AB: process exits
    AB->>FS: capture git status + diff
    AB->>AB: classify completion
    Caller->>AB: tools/call agent_evidence
    AB-->>Caller: review packet (status, diff, changedFiles, recommendations)
    Caller->>FS: run local verification / inspect managed worktree
```

**Accuracy notes:** Verified against `router_runtime.rs`, `mcp_adapter.rs`, `spawn.rs`, and `complete.rs`.

### Recover from a Stalled Turn

**Actors:** Primary Agent, Agent Bridge MCP
**Trigger:** The routed turn returns a blocker/failure or times out

```mermaid
sequenceDiagram
    participant Caller as Primary Agent
    participant AB as Agent Bridge MCP

    Caller->>AB: session/prompt or agent_delegate
    AB-->>Caller: blocker/failure + evidenceRef
    Caller->>AB: agent_evidence sections:[stdout,stderr,transcript]
    AB-->>Caller: bounded logs + diagnostic
    Caller->>Caller: decide whether to retry with a narrower prompt
```

**Accuracy notes:** Matches router terminal classification, `review.rs` result shaping, and `supervision.rs` termination logic.

### Claude-Host-Runner Setup

**Actors:** Human Operator, Claude Host Runner, Agent Bridge MCP
**Trigger:** Human wants to use Claude provider

```mermaid
sequenceDiagram
    participant Op as Human Operator
    participant HR as Claude Host Runner
    participant AB as Agent Bridge MCP
    participant Claude as Claude Code

    Op->>HR: agent-bridge-mcp claude-host-runner /sock/path
    HR->>HR: bind Unix socket
    Note over HR: awaits requests
    Op->>AB: configure Agent Bridge with AGENT_BRIDGE_CLAUDE_HOST_SOCKET=/sock/path
    AB->>AB: --doctor-smoke --provider <name>
    AB->>HR: ping request
    HR-->>AB: pong
    AB-->>Op: readiness OK
```

**Accuracy notes:** Derived from `claude_host.rs` protocol and `provider.rs` readiness contract.

## Tenant-Specific Behavior

Not applicable. Single-tenant desktop tool.
