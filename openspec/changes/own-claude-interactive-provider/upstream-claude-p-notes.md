## Upstream claude-p Source Notes

Sources inspected from `smithersai/claude-p` `main`:

- `SPEC.md`
- `README.md`
- `src/driver.zig`
- `src/hook.zig`
- `src/terminal.zig`
- `src/transcript.zig`
- `src/stream.zig`
- `src/emit.zig`

## PTY Startup and Readiness

`claude-p` runs the official interactive `claude` binary in a real PTY through
`zmux.NativeSession`. The child command is built as normal Claude CLI arguments
plus inline `--settings`, then shell-quoted into a `/bin/sh -c` command for the
PTY session. The environment is inherited, with `CLAUDE_P_FIFO` set to the
temporary hook FIFO and `TERM=xterm-256color`.

The driver starts a reader thread through `zmux` and treats UI readiness as a
quiescence problem rather than a single prompt marker. It waits for terminal
output to go silent for a short window, bounded by a startup maximum, before
sending the prompt. It also sends the prompt bytes and Enter separately, with a
small delay, because Claude/Ink can otherwise treat a burst as paste-like input
and leave the carriage return in the input buffer.

Port implication: Agent Bridge should keep the real PTY and quiescence model,
but should not copy the shell-string command construction as the host-runner
boundary. The owned host-runner protocol should keep requests structured and
use only a fixed runner launch path.

## Terminal Probe Responses

`src/terminal.zig` implements a stateless scanner over PTY output and appends
responses for Claude Code startup probes:

- DA1: `ESC[c` / `ESC[0c` -> `ESC[?1;2c`
- DA2: `ESC[>c` / `ESC[>0c` -> `ESC[>0;0;0c`
- DSR cursor position: `ESC[6n` -> `ESC[1;1R`
- XTVERSION: `ESC[>q` / `ESC[>0q` -> `ESC P>|claude-p ESC\`
- window size: `ESC[18t` -> `ESC[8;40;120t`

The reader callback queues those response bytes and the main loop writes them
back to the PTY. The code intentionally avoids re-entering `zmux` from the
reader callback.

Port implication: keep terminal response logic isolated, test it against
fragmented chunks, and write responses only to the PTY input side. Probe bytes
must never be forwarded to MCP stdout or provider result parsing.

## Hook Command Protocol

`src/hook.zig` creates a temporary directory named `claude-p-<pid>-<random>`
under `$TMPDIR` or `/tmp`, then creates:

- a FIFO with mode `0600`;
- an executable relay script with mode `0700`;
- inline settings JSON registering `SessionStart` and `Stop` hooks.

The relay script receives the event name as argv, reads the hook JSON payload
from stdin, and appends one tab-delimited line to the FIFO:

```text
<event>\t<payload-json>\n
```

The driver opens the FIFO for reading before spawning Claude so the hook writer
does not block indefinitely. `SessionStart` is used to capture early transcript
metadata and trigger prompt injection after UI quiescence. `Stop` is used as the
completion signal and final transcript source.

Port implication: use the same broad shape, but make it Agent Bridge owned:
owner-only temp directory, owner-only relay artifacts, bounded hook line reads,
Bridge-specific environment names, and cleanup on every exit path. Also add
`StopFailure`, which upstream does not handle.

## Transcript Parsing and Output

`src/transcript.zig` parses Claude transcript JSONL and skips malformed lines
rather than failing the entire parse. It extracts:

- final assistant text from the last assistant message text blocks;
- `sessionId` or `session_id`;
- usage from assistant message usage fields;
- result metadata from `type: "result"` records when present.

After a `Stop` hook, `src/driver.zig` retries transcript parsing up to a bounded
flush budget before falling back to the Stop payload's `last_assistant_message`
and session id. `src/stream.zig` tails transcript JSONL with `pread`, buffers
partial lines until newline, and flushes a final partial line at session end.
`src/emit.zig` renders text, json, and stream-json output shapes.

Port implication: Agent Bridge does not need public `claude -p` byte-for-byte
format parity, but it should keep the transcript priority order: transcript
JSONL first, bounded retry for flush races, Stop `last_assistant_message`
fallback second, and explicit diagnostics for malformed/missing transcript data.

## Timeout and Cleanup

The upstream driver loops around FIFO events, queued terminal responses,
optional stream pumping, timeout checks, and child liveness. On timeout it
returns a timeout error and terminates the PTY session. The upstream spec states
that termination sends SIGTERM and escalates to SIGKILL after a short grace
period via `zmux`.

`HookHarness.deinit` best-effort deletes the FIFO, relay script, and temporary
directory. The driver also terminates the session after successful Stop parsing.

Port implication: Agent Bridge should make cleanup part of the owned runner
contract: timeout, client disconnect, MCP shutdown, StopFailure, malformed
transcript, and successful completion all need deterministic child and temp-file
cleanup. Process cleanup should cover the child process group, not just the
immediate shell or command wrapper.

## Behaviors Not To Copy Directly

- Do not preserve the upstream CLI compatibility boundary; Agent Bridge only
  needs provider task execution.
- Do not keep `claude-p` naming in environment variables, diagnostics, or
  provider metadata.
- Do not accept hook transcript paths blindly. The bridge spec requires
  canonicalization, regular-file checks, symlink rejection after
  canonicalization, bounded reads, and safe fallback behavior.
- Do not rely on `SessionStart`/`Stop` only. Owned Claude must classify
  `StopFailure` payloads for API/auth/billing/rate-limit failures.
- Do not treat version discovery or process spawn as launch readiness. The
  provider is launchable only after the owned runner reaches Stop/transcript
  completion during smoke.
- Do not copy the generic shell command construction into the host-runner
  protocol. The host runner should accept structured Claude requests, not
  arbitrary command strings.
