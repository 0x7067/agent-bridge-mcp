## PTY Adapter Decision

## Decision

Use `pty-process = { version = "0.5.3", features = ["async"] }` behind an
Agent Bridge owned adapter module.

Do not expose `pty-process` types outside the adapter. The owned adapter should
define the runner-facing operations:

- open a PTY at a fixed size;
- spawn the official interactive `claude` command with structured argv/env/cwd;
- split PTY read/write for concurrent Tokio tasks;
- resize if needed;
- expose the child pid for process-group cleanup;
- wait/terminate with the same timeout and shutdown semantics as existing
  provider process handling.

## Sources Checked

- `pty-process` 0.5.3 docs: `https://docs.rs/pty-process/latest/pty_process/`
- `pty-process::Command` docs:
  `https://docs.rs/pty-process/latest/pty_process/struct.Command.html`
- `pty-process::Pty` docs:
  `https://docs.rs/pty-process/latest/pty_process/struct.Pty.html`
- `portable-pty` 0.9.0 docs:
  `https://docs.rs/portable-pty/latest/portable_pty/`
- `tokio-pty-process` 0.4.0 docs:
  `https://docs.rs/tokio-pty-process/latest/tokio_pty_process/`
- `nexcore-pty` 0.1.0 docs:
  `https://docs.rs/nexcore-pty/latest/nexcore_pty/`
- Local repo dependency/runtime shape: `tokio` runtime, existing Unix
  process-group cleanup through `libc::setpgid`/`killpg`, and no current PTY
  dependency.

## Evaluation Matrix

| Option | macOS/Linux support | Tokio integration | Cleanup fit | Maintenance risk | Decision |
| --- | --- | --- | --- | --- | --- |
| `pty-process` 0.5.3 | Docs build for Linux and macOS; POSIX PTY model fits this project target. | Direct: async API wraps `tokio::process::Command`; PTY implements `AsyncRead` and `AsyncWrite`; can split owned halves for tasks. | Good: spawned child is a normal `tokio::process::Child`; docs state child becomes session leader with PTY as controlling terminal. Existing `killpg(pid)` cleanup should fit. | Moderate: smaller crate than `portable-pty`, but current docs show modern Tokio 1.x and rustix dependencies. | Selected. |
| `portable-pty` 0.9.0 | Strong: docs include `aarch64-apple-darwin`, `aarch64-unknown-linux-gnu`, Linux, and Windows. | Indirect: synchronous `Read`/`Write`; needs blocking threads or `spawn_blocking` bridge. | Good child API includes `process_id`, `try_wait`, `wait`, and child killing. | Lower: part of WezTerm and broader platform coverage. | Keep as fallback if `pty-process` fails on macOS arm64 or cleanup behavior. |
| `tokio-pty-process` 0.4.0 | Unix only, docs build on macOS/Linux. | Old Tokio 0.1/futures 0.1 stack. | Old child implementation copied from pre-1.0 Tokio process era. | High: last published 2019; obsolete dependency stack. | Reject. |
| `nexcore-pty` 0.1.0 | POSIX macOS/Linux. | Synchronous primitives only; would require custom nonblocking integration. | Exposes low-level fork/exec and signal primitives. | High: new/small crate, unusual docs, little adoption signal. | Reject for now. |
| In-house `libc`/`openpty` adapter | Can target exactly macOS/Linux. | We would need to implement nonblocking registration and process/session handling ourselves. | Full control, but easy to get fork/exec/session cleanup wrong. | High implementation risk and audit cost. | Keep only if crates fail. |

## Why `pty-process`

This repo is already a Tokio-first Rust binary. The owned Claude runner needs
concurrent PTY reading, prompt writes, timeout handling, hook IPC, and host
runner disconnect cleanup. `pty-process` fits that shape because:

- `Command` wraps `tokio::process::Command`;
- `Pty` implements `AsyncRead` and `AsyncWrite`;
- `Pty::into_split` supports moving read/write halves into independent tasks;
- `Pty::resize` covers terminal size control;
- spawned children are session leaders with the PTY as controlling terminal;
- the returned child stays compatible with current timeout/wait patterns.

`portable-pty` has stronger battle-tested platform provenance, but adding it
would push the runner toward blocking reader/writer threads. That is acceptable
as a fallback, but not the first implementation for this Tokio codebase.

## Adapter Constraints

- Keep all PTY crate usage in one module, tentatively
  `claude_interactive::pty`.
- Add tests around the adapter using a fake interactive command before wiring
  real Claude.
- Do not interpolate command paths, settings paths, model, effort, cwd, or prompt
  text into shell source. Use structured argv/env/cwd.
- Preserve prompt redaction: rendered prompts go to PTY input only.
- On Unix cleanup, prefer process-group termination using the child pid as the
  process group leader after verifying the `pty-process` spawn semantics in
  tests.
- If macOS arm64 build/runtime behavior fails, switch the adapter internals to
  `portable-pty` without changing runner-facing types.
