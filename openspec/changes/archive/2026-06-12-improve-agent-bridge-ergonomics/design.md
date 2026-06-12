## Context

Today Agent Bridge derives all tunables from environment variables:
- `AGENT_BRIDGE_WORKSPACES` – parsed in `claude_host.rs` and `server.rs` independently
- `AGENT_BRIDGE_STATE_DIR` – expanded in `runtime.rs` via `expand_home`
- `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` – probed ad hoc in `server.rs` probes
- `AGENT_BRIDGE_MAX_ACTIVE_TASKS` – read in `task.rs` via `env::var(...)`

There is no typed `Config` struct, so validation logic is duplicated and unit-tested only indirectly. The binary entrypoint (`main.rs`) ignores all arguments except the magic `claude-host-runner <socket>` subcommand, providing no `--help`, `--version`, or pre-flight diagnostics. Progress observation (`agent_observe`) spins a 50 ms sleep loop, constantly reopening and re-reading `transcript.jsonl` from disk until new events appear. Logs are unstructured `eprintln!` lines mixed with provider stderr dumps, making correlation across the actor, launcher, and IO drainers impossible without manual file grepping.

## Goals / Non-Goals

**Goals:**
1. Establish a single, typed, validated `Config` struct sourced from layered inputs (defaults < file < env < CLI).
2. Give the binary a minimal standalone CLI surface (`--help`, `--version`, `--config-check`, `--doctor-smoke`).
3. Eliminate the busy-wait polling loop in `agent_observe` by switching to an internal event-driven subscription model, with a file-watcher fallback for thin clients.
4. Instrument the codebase with `tracing` spans/events, emitting JSON exclusively to stderr so stdout remains the pristine MCP transport.
5. Allow runtime reload of workspace roots without restarting the server process.

**Non-Goals:**
- Rewriting the MCP protocol layer or adding new MCP methods.
- Changing the eight-tool public surface (beyond additive CLI conveniences).
- Replacing the existing `doctor` MCP tool contract; `--doctor-smoke` is a CLI wrapper around the same engine.
- Adding retry policies, weighted concurrency, or provider-output validators (those belong to the companion `improve-delegation-output-quality` change).

## Decisions

### D1: TOML for configuration file format
**Decision:** Use TOML as the sole supported config file format.
**Rationale:** TOML is native to the Rust ecosystem (used by Cargo), supports comments, and avoids indentation fragility. YAML is already absent from the repo. JSON is unsuitable for human-edited configs.
**Alternative considered:** YAML – rejected due to parser complexity and lack of project precedent. Environment-only – rejected because this perpetuates the existing ergonomics gap.

### D2: `figment` for config layering
**Decision:** Use the `figment` crate (or hand-roll a light-weight layered loader) to compose `Defaults → Config File → Env Vars → CLI Flags`.
**Rationale:** `figment` provides exactly this layered paradigm with `Serialized` profiles, and is widely adopted (e.g., Rocket). If minimizing deps is preferred, a hand-written 80-line loader using `serde` + `toml` + ` envy`-style prefix stripping is equally viable.
**Alternative considered:** Pure `clap` with env-value fallbacks – rejected because it does not support file-based config and would require duplicating field declarations.

### D3: `clap` for CLI parsing
**Decision:** Use `clap` with derive macros.
**Rationale:** Zero additional learning curve for Rust engineers; generates `--help` automatically; integrates natively with `figment` for env/CLI fusion. The binary is already published (`cargo install` friendly), so CLI polish is warranted.
**Trade-off:** Increases binary size slightly (~few hundred KB). Negligible compared to the Tokio runtime already linked.

### D4: Event-driven observe via `tokio::sync::watch` + optional `notify` file watcher
**Decision:** Replace the 50 ms sleep loop with an internal `tokio::sync::watch` channel owned by the `TaskActor`. Each `drain_log` task signals the watch channel when new transcript lines are appended. `agent_observe` awaits the watch receiver instead of sleeping. For thin clients that do not hold an open subscription, an optional `notify`-based watcher on `transcript.jsonl` wakes the poller.
**Rationale:** Removes the hottest syscall loop in the codebase. `tokio::sync::watch` is lock-free and purpose-built for “many readers, one writer” broadcast scenarios. `notify` is only needed for the external fallback path and can be gated behind a feature flag.
**Alternative considered:** Native MCP notifications (`notifications/message`) – deferred because client support is uneven; can be added atop the watch channel later without redesign.

### D5: `tracing` with JSON subscriber on stderr
**Decision:** Adopt `tracing` and `tracing-subscriber` formatted as newline-delimited JSON written to stderr.
**Rationale:** Preserves the invariant that stdout is the MCP JSON-RPC channel. JSON simplifies ingestion into jq/log aggregators. Spans carry `agent_id`, `provider`, `mode`, and `task_status` so a single `trace_id` correlates actor decisions, child launches, and IO completions.
**Trade-off:** Every `eprintln!` must be migrated. Risk of accidentally leaving a stray print in a new PR is mitigated by a `#[deny(print_stdout)]` lint (clippy restriction).

### D6: Explicit `reload` subcommand over implicit file watching
**Decision:** Expose a CLI subcommand `agent-bridge-mcp reload` that sends `SIGHUP` or a Unix socket signal to the running server, triggering workspace-root revalidation.
**Rationale:** Simpler than integrating the `notify` crate for file watching. Gives the operator explicit control over when the reload occurs, avoiding races during partial config edits. Matches classic daemon ergonomics (nginx, sshd).
**Alternative considered:** `notify` crate watching the config file – rejected because it adds a dependency and potential noise from editors’ swap/write patterns. May be revisited in the future.

## Risks / Trade-offs

- **[Breaking?] No.** All changes are additive. The MCP tool surface gains no new required arguments; CLI additions are orthogonal to the stdio server loop.
- **[Risk]** Misconfiguring `tracing` to write to stdout would corrupt the MCP transport.
  → Mitigation: Unit test asserting that `tracing_subscriber::fmt::layer().with_writer(std::io::stderr)` is initialized correctly; CI gate that greps for `println!` and `print!`.
- **[Risk]** Switching `agent_observe` to event-driven internals could introduce a missed-wakeup race if the watch-channel sender is dropped before observers receive the final event.
  → Mitigation: Always fire a sentinel `watch` update after the child completes and after the transcript is flushed. Observers timing out on the channel receive the sentinel and perform a final read.
- **[Risk]** `clap` and `figment` additions may trigger `cargo machete` false positives if not wired correctly.
  → Mitigation: Run `scripts/quality.sh` after every artifact; machete is a hard gate.
- **[Risk]** Reloading workspace roots while active tasks are running may invalidate `cwd`s that were previously legal.
  → Mitigation: Reload only expands the root set; shrinking requires a full restart. Document this behavior.

## Migration Plan

1. Land the `Config` refactor first, maintaining env-var parity so existing setups are unaffected.
2. Add CLI parsing (`--help`, `--version`, `--config-check`).
3. Swap the `agent_observe` sleep loop for the watch channel; keep the 50 ms path behind a `legacy-sync-poller` compilation flag for emergency rollback.
4. Introduce `tracing` spans incrementally: start with `TaskActor` and `launch_task`, then propagate outward.
5. Add `reload` subcommand.
6. Archive deprecated env-only documentation once the config file is documented.

## Open Questions

- Should we preserve a `warn_on_deprecated_env_vars` shim for `AGENT_BRIDGE_WORKSPACES` et al., or cut over immediately? Recommendation: preserve with a deprecation log for one release cycle.
- Which `figment` alternative minimizes deps if `cargo machete` sensitivity is a concern? A hand-rolled `LayeredConfig` struct with `serde` + `toml` + ` envy` is approximately 100 lines and zero net-new deps beyond `toml`.
