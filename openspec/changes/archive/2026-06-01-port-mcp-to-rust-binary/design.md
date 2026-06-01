## Context

The final runtime is now the Rust `agent-bridge-mcp` binary; the former runtime has been removed.

The Rust refactor has two primary objectives:

- **Type safety:** replace runtime string/object validation with typed protocol, task, provider, error, and state models.
- **Single binary:** ship the MCP server as a built `agent-bridge-mcp` executable.

Agent input converged on the same constraint: this was a compatibility migration, not a greenfield rewrite. Legacy behavior was captured during the migration, and ongoing verification is Rust-only.

Official MCP docs define stdio as UTF-8 JSON-RPC messages over stdin/stdout, newline-delimited, with no non-MCP stdout output. The official SDK docs list Rust as a Tier 2 SDK, so the implementation should evaluate `rmcp` but not let SDK convenience override exact compatibility with the current wire shape.

## Goals / Non-Goals

**Goals:**
- Produce a Rust MCP server binary that preserves the public task lifecycle API.
- Use typed Rust models for providers, task modes, task statuses, task phases, error types, isolation, command descriptors, tool inputs, tool outputs, and persisted task records.
- Build Rust stdio and lifecycle tests that preserve the migrated public behavior.
- Preserve provider adapter behavior, including Claude shell initialization, provider command args, env allowlists, provider checks, and smoke probes.
- Preserve or explicitly migrate existing task registry and task directory state.
- Keep the final user-facing MCP runtime as one built executable named `agent-bridge-mcp`.
- Remove side-by-side execution after Rust parity is proven.

**Non-Goals:**
- No public tool rename or task API redesign.
- No plugin system for providers.
- No rewrite of provider CLIs; the binary still delegates to local external CLIs.
- No HTTP MCP transport in this change.
- No guarantee that all external providers work cross-platform; the binary can be cross-platform while provider CLIs may not be.

## Decisions

1. **Compatibility fixtures first.**

   Before implementing substantial Rust behavior, extract the current public behavior into stdio fixtures. After the final switch, keep equivalent coverage in Rust-only tests against:

   ```text
   target/debug/agent-bridge-mcp
   ```

   Tests should cover `initialize`, `tools/list`, `providers_list`, `providers_check`, validation errors, `task_preview`, lifecycle states, `task_logs`, `task_result`, stale startup recovery, and managed worktree cleanup failure. This addresses the agents' strongest warning: implementation-module tests alone do not prove MCP stdio behavior.

   Fixture comparison should be semantic, not brittle byte-for-byte comparison. Normalize dynamic fields such as task IDs, timestamps, durations, process IDs, map ordering, and environment key ordering. Do not normalize away command arguments, error text/type, required fields, schema properties, cap/truncation flags, or state transitions.

2. **Use a single Rust binary crate, not a workspace or framework-first rewrite.**

   Start with one focused binary crate using `tokio`. A workspace can be introduced later only if the single crate becomes genuinely hard to test. `tokio` is the default because stdio, child-process management, timers, and async pipe draining are central to this server. Suggested module layout:

   ```text
   crates/agent-bridge-mcp/
     Cargo.toml
     src/
       main.rs
       mcp.rs
       tools.rs
       error.rs
       provider/
         mod.rs
         claude.rs
         cursor.rs
         kimi.rs
         codex.rs
       task/
         mod.rs
         manager.rs
         state.rs
         logs.rs
         worktree.rs
       storage.rs
       process.rs
       git.rs
       paths.rs
       env.rs
   ```

   A top-level workspace can be added if useful, but the first implementation should stay one binary crate unless tests or packaging need more separation.

3. **Serialize task state access through a non-blocking actor.**

   The Rust `TaskManager` should use one explicit model:

   - required: a single task-manager actor that receives commands over channels and owns the registry plus active task map.

   The actor model matches the current single-writer semantics and avoids lock-order problems when process events, waits, stops, and result reads happen concurrently. The actor must not await long-running child processes, git commands, or worktree cleanup directly. Those operations should run in background tasks and report completion back to the actor through messages. The actor loop should continue servicing independent requests while background work is in progress.

   Actor failure policy: if the actor panics, abort the server process rather than leaving request handlers blocked on dropped response channels. This is harsher than restart, but it is easier to reason about and avoids a silently unhealthy MCP server. The implementation must not rely on Tokio's default spawned-task panic behavior, because a panic in a detached spawned task can otherwise leave the runtime alive. The actor `JoinHandle` must be monitored by a watchdog that calls `std::process::abort()` on panic, or the binary must use an equivalent process-wide abort strategy. Panic tests must verify diagnostics on stderr and a non-zero process exit.

   Actor command and completion channels must be bounded. Public request handlers should apply backpressure by awaiting channel capacity instead of buffering unbounded work. Background completion senders must not silently drop final state updates; if the actor cannot accept completion messages, the server should surface a clear internal error or fail fast rather than losing lifecycle state.

   The initial Rust port preserves the current unbounded task concurrency behavior for compatibility. A maximum concurrent task limit can be introduced later as a separate safety change after public behavior has parity fixtures.

4. **Guard MCP stdout.**

   The Rust binary must reserve stdout exclusively for MCP JSON-RPC messages. Configure logging/tracing for stderr only, avoid `println!`, and install a panic hook that writes panic diagnostics to stderr. Tests should include a stdout discipline check so panics/logging cannot accidentally corrupt the MCP stream.

5. **Use local protocol DTOs unless the Rust SDK preserves exact behavior.**

   Evaluate the official Rust MCP SDK early. If it can preserve exact `tools/list`, `tools/call`, content, error, and stdio behavior, use it. If not, implement the small required MCP JSON-RPC surface locally with `serde`, `serde_json`, and async stdio. The current server only needs initialize, initialized notifications, tools/list, and tools/call.

6. **Separate public DTOs from validated domain types.**

   Public request structs should deserialize with unknown-field rejection. They should validate into internal types:

   ```rust
   ProviderKind
   TaskMode
   TaskStatus
   TaskPhase
   ErrorType
   SafeCwd
   Prompt
   TimeoutSeconds
   Isolation
   WorktreeName
   ProviderCommand
   ```

   Persisted records should use `serde(rename_all = "camelCase")` to preserve current JSON field names.

7. **Provider adapters remain fixed and typed.**

   Use an enum-dispatch facade or trait objects for provider adapters. Each adapter owns:

   - capability metadata
   - supported mode and option validation
   - command construction
   - version command
   - smoke command
   - environment allowlist

   This keeps the Rust implementation aligned with the archived `provider-adapter-contract` spec.

8. **Keep persisted state inspectable.**

   Current `registry.json` has no schema version. Completed tasks must remain inspectable. Previously `queued` or `running` tasks must still become `failed_stale` at startup. Startup should also clean up or ignore known temporary registry files left by crashed atomic writes.

   Registry deserialization should tolerate unknown fields. Strict unknown-field rejection belongs on public tool request DTOs, not persisted state. Task IDs should keep the existing `task_` plus UUID-v4 hex-without-hyphens shape and retry on collision if an ID already exists in the registry.

   Atomic writes are required on the supported targets. Temporary registry files must be created in the same directory as the canonical `registry.json` before rename, so replacement stays atomic on one filesystem. If the canonical `registry.json` is present but corrupted, the Rust server must fail startup with a clear stderr diagnostic instead of silently starting with empty state; missing registry files still initialize to an empty registry. First release targets are macOS arm64, macOS x64, and Linux x64. Windows support is explicitly out of scope for the first Rust migration unless a Windows-safe atomic write and process model are selected before implementation.

9. **Single binary final shape.**

   The long-term MCP command should be the built binary. The packaging path is direct prebuilt binary release for supported targets. The transition may expose a temporary `agent-bridge-mcp-rs`, but final docs should point at `agent-bridge-mcp`.

10. **Preserve remove semantics for active tasks.**

   Current `task_remove` rejects running or queued tasks and tells callers to stop first. Rust should preserve that public behavior instead of implicitly stopping and removing active tasks.

11. **Preserve current stdio shutdown semantics.**

   The server exits when stdin reaches EOF and ignores unknown notifications without a response. Do not add behavior for `notifications/exit` unless a fixture first defines the desired public behavior and a compatibility reason exists.

12. **Preserve signal child cleanup.**

   The Rust binary sends `SIGTERM` to tracked active provider children on `SIGINT` or `SIGTERM` for supported Unix targets. Do not use Rust's default child-kill primitive as the compatibility mechanism unless it is proven to send `SIGTERM` on the supported target; use a Unix signal helper (`libc` or equivalent selected during implementation) that sends `SIGTERM` to the tracked provider PID. After sending termination, the background process task must continue to await child exit so the process is reaped and does not become a zombie. Shutdown handling must wait for active-child cleanup for a bounded grace period, defaulting to 5 seconds, then escalate remaining children to SIGKILL and continue reaping before exiting the server process.

13. **Reject non-Rust single-binary compilers for this change.**

   Tools that package an existing dynamic runtime into a standalone executable could satisfy part of the single-executable goal, but they do not address the type-safety objective. They also preserve the same runtime validation and stringly state model. This change should use Rust unless implementation discovers a blocker severe enough to revisit the language decision.

## Risks / Trade-offs

- **Risk: protocol drift** -> Mitigation: Rust stdio fixtures become the migration gate.
- **Risk: SDK mismatch** -> Mitigation: evaluate `rmcp` behind fixtures; fall back to local JSON-RPC DTOs if exact compatibility is hard.
- **Risk: state migration loses inspectability** -> Mitigation: keep persisted records tolerant of unknown fields and avoid implicit destructive migrations.
- **Risk: provider process deadlock** -> Mitigation: log readers keep draining stdout/stderr even after cap.
- **Risk: task state races under Rust async runtime** -> Mitigation: choose an actor or single-lock state model before implementing lifecycle methods.
- **Risk: actor blocks all task APIs on one long operation** -> Mitigation: actor owns state only; process/git/worktree operations run in background tasks and report completion by message.
- **Risk: actor panic leaves server alive but unresponsive** -> Mitigation: actor panic aborts the server process.
- **Risk: Rust diagnostics corrupt MCP stdout** -> Mitigation: configure stderr-only tracing/logging and panic reporting before protocol handlers are implemented.
- **Risk: cross-platform storage assumptions corrupt registry** -> Mitigation: first release targets exclude Windows unless a Windows-safe atomic write strategy is chosen.
- **Risk: single binary expectation is overstated** -> Mitigation: document that the MCP server is one binary, while provider CLIs and `git` remain external dependencies.
- **Risk: cross-platform surprises** -> Mitigation: define supported release targets early; treat `/bin/zsh`, Unix signals, and provider availability as platform-specific behavior.
- **Risk: side-by-side transition becomes permanent** -> Mitigation: make `agent-bridge-mcp-rs` temporary and require final replacement or explicit rollback.
- **Risk: false fixture parity** -> Mitigation: define normalization rules before writing fixtures and include exact comparison of schemas, command args, errors, and states.

## Migration Plan

1. Capture the legacy public behavior in stdio fixtures before removing the legacy runtime.
2. Add Rust crate skeleton, runtime choice, stderr-only diagnostics, typed DTO/domain modules, and minimal `initialize`/`tools/list`.
3. Run fixtures against the minimal Rust skeleton and record expected failures before implementing green behavior.
4. Implement provider adapter parity in Rust and prove with command/env fixtures.
5. Implement task storage and lifecycle parity with fake provider binaries using a non-blocking task-manager actor and completion-message model.
6. Implement process/log/git/worktree behavior.
7. Add packaging path and built/installed binary smoke tests.
8. Audit public API parity and document either "no break" or exact migration notes.
9. Switch the primary `agent-bridge-mcp` entrypoint to the Rust binary after fixture parity, `cargo test`, Rust lifecycle tests, and direct smoke checks pass.
10. Remove the former runtime code after the Rust binary is the final server.

Rollback: use version control or a released artifact if a rollback is needed. The repo no longer keeps the former MCP server.

## Open Questions

- None.
