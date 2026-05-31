## Context

The server is currently a dependency-free Node.js stdio MCP server. The provider-specific surface is now isolated in `src/provider-registry.mjs`, while `src/server.mjs` still owns protocol routing, tool dispatch, task lifecycle, persistence, process management, logs, git snapshots, worktree cleanup, and stdio transport.

The Rust refactor has two primary objectives:

- **Type safety:** replace runtime string/object validation with typed protocol, task, provider, error, and state models.
- **Single binary:** ship the MCP server as a built `agent-bridge-mcp` executable instead of requiring Node.js to run the server.

Agent input converged on the same constraint: this must be a compatibility migration, not a greenfield rewrite. The Node implementation and its current tests are the reference behavior until golden stdio fixtures prove Rust parity.

Official MCP docs define stdio as UTF-8 JSON-RPC messages over stdin/stdout, newline-delimited, with no non-MCP stdout output. The official SDK docs list Rust as a Tier 2 SDK, so the implementation should evaluate `rmcp` but not let SDK convenience override exact compatibility with the current wire shape.

## Goals / Non-Goals

**Goals:**
- Produce a Rust MCP server binary that preserves the public task lifecycle API.
- Use typed Rust models for providers, task modes, task statuses, task phases, error types, isolation, command descriptors, tool inputs, tool outputs, and persisted task records.
- Build a golden stdio fixture suite that runs against both Node and Rust.
- Preserve provider adapter behavior, including Claude shell initialization, provider command args, env allowlists, provider checks, and smoke probes.
- Preserve or explicitly migrate existing task registry and task directory state.
- Keep the final user-facing MCP runtime as one built executable named `agent-bridge-mcp`.
- Keep side-by-side Node/Rust execution temporary and test-oriented.

**Non-Goals:**
- No public tool rename or task API redesign.
- No plugin system for providers.
- No rewrite of provider CLIs; the binary still delegates to local external CLIs.
- No HTTP MCP transport in this change.
- No guarantee that all external providers work cross-platform; the binary can be cross-platform while provider CLIs may not be.

## Decisions

1. **Compatibility fixtures first.**

   Before implementing substantial Rust behavior, extract the current public behavior into stdio fixtures that can execute against:

   ```text
   node src/server.mjs
   target/debug/agent-bridge-mcp
   ```

   Fixtures should cover `initialize`, `tools/list`, `providers_list`, `providers_check`, validation errors, `task_preview`, lifecycle states, `task_logs`, `task_result`, stale startup recovery, and managed worktree cleanup failure. This addresses the agents' strongest warning: direct Node module tests will not prove Rust parity.

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

   Node currently gets mutation serialization from the single-threaded event loop. Rust must not rely on accidental mutex usage scattered across handlers. The Rust `TaskManager` should use one explicit model:

   - required: a single task-manager actor that receives commands over channels and owns the registry plus active task map.

   The actor model matches the current single-writer semantics and avoids lock-order problems when process events, waits, stops, and result reads happen concurrently. The actor must not await long-running child processes, git commands, or worktree cleanup directly. Those operations should run in background tasks and report completion back to the actor through messages. The actor loop should continue servicing independent requests while background work is in progress.

   Actor failure policy: if the actor panics, abort the server process rather than leaving request handlers blocked on dropped response channels. This is harsher than restart, but it is easier to reason about and avoids a silently unhealthy MCP server.

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

8. **Keep persisted state rollback-compatible before replacing Node.**

   Current `registry.json` has no schema version. Before the final entrypoint switch, Rust must write a Node-readable registry shape so rollback to the Node server remains possible. A versioned migration can be introduced only behind an explicit post-switch migration flag or after the Node rollback path is no longer required. Completed tasks must remain inspectable. Previously `queued` or `running` tasks must still become `failed_stale` at startup. Startup should also clean up or ignore known temporary registry files left by crashed atomic writes.

   Registry deserialization should tolerate unknown fields. Strict unknown-field rejection belongs on public tool request DTOs, not persisted state. Task IDs should keep the existing `task_` plus UUID-v4 hex-without-hyphens shape and retry on collision if an ID already exists in the registry.

   Atomic writes are required on the supported targets. First release targets are macOS arm64, macOS x64, and Linux x64. Windows support is explicitly out of scope for the first Rust migration unless a Windows-safe atomic write and process model are selected before implementation.

9. **Single binary final shape, npm optional as distribution wrapper.**

   The long-term MCP command should be the built binary. The first packaging path is direct prebuilt binary release for supported targets. If preserving npm install UX matters after that, npm should distribute or launch platform-specific prebuilt binaries rather than run the server through Node. The transition may expose a temporary `agent-bridge-mcp-rs`, but final docs should point at `agent-bridge-mcp`.

10. **Preserve remove semantics for active tasks.**

   Current `task_remove` rejects running or queued tasks and tells callers to stop first. Rust should preserve that public behavior instead of implicitly stopping and removing active tasks.

11. **Preserve current stdio shutdown semantics.**

   Current Node behavior exits when stdin reaches EOF and ignores unknown notifications without a response. The Rust implementation should preserve that baseline. Do not add behavior for `notifications/exit` unless a fixture first defines the desired public behavior and a compatibility reason exists.

12. **Preserve signal child cleanup.**

   Current Node behavior sends `SIGTERM` to tracked active provider children on `SIGINT` or `SIGTERM`. The Rust binary should keep equivalent cleanup for supported Unix targets.

13. **Reject Node single-binary compilers for this change.**

   Tools that package JavaScript into a standalone executable could satisfy part of the single-executable goal, but they do not address the type-safety objective. They also preserve the same runtime validation and stringly state model. This change should use Rust unless implementation discovers a blocker severe enough to revisit the language decision.

## Risks / Trade-offs

- **Risk: protocol drift** -> Mitigation: golden stdio fixtures become the migration gate; Rust cannot replace Node until fixture parity passes.
- **Risk: SDK mismatch** -> Mitigation: evaluate `rmcp` behind fixtures; fall back to local JSON-RPC DTOs if exact compatibility is hard.
- **Risk: state migration loses inspectability or rollback** -> Mitigation: keep Rust registry writes Node-readable until final switch; gate any versioned migration behind a post-switch flag.
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

1. Add stdio fixture harness for the current Node implementation.
2. Add Rust crate skeleton, runtime choice, stderr-only diagnostics, typed DTO/domain modules, and minimal `initialize`/`tools/list`.
3. Run fixtures against the minimal Rust skeleton and record expected failures before implementing green behavior.
4. Implement provider adapter parity in Rust and prove with command/env fixtures.
5. Implement task storage and lifecycle parity with fake provider binaries using a non-blocking task-manager actor and completion-message model.
6. Implement process/log/git/worktree behavior.
7. Add packaging path and built/installed binary smoke tests.
8. Audit public API parity and document either "no break" or exact migration notes.
9. Switch the primary `agent-bridge-mcp` entrypoint to the Rust binary after fixture parity, `cargo test`, Node compatibility tests, and direct smoke checks pass.
10. Remove or demote Node runtime code only after a rollback point is available.

Rollback: keep the Node server runnable until the final switch. If Rust parity or packaging fails, continue shipping the Node entrypoint while the Rust binary remains experimental.

## Open Questions

- Should the npm install UX remain supported with prebuilt platform binaries, or should distribution move to direct binary releases first?
- Should the Rust implementation add a maximum concurrent task limit during the port, or preserve unbounded behavior until a later safety change?
