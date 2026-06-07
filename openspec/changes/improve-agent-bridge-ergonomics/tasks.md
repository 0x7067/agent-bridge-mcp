## 1. Foundations

- [ ] 1.1 Add `toml` and `clap` dependencies to `Cargo.toml` with minimal feature flags
- [ ] 1.2 Create `src/config.rs` with a `Config` struct deserializable from TOML, env, and CLI
- [ ] 1.3 Move `expand_home` and `configured_workspace_roots` into `config.rs` as methods on `Config`
- [ ] 1.4 Add deprecation shim for legacy env vars (`AGENT_BRIDGE_WORKSPACES`, `AGENT_BRIDGE_STATE_DIR`, etc.) with `tracing::warn!` notices
- [ ] 1.5 Run `cargo machete` and `scripts/quality.sh` to verify no unused deps or lints

## 2. CLI Surface

- [ ] 2.1 Derive `clap` CLI struct with `--help`, `--version`, `--config-check`, `--doctor-smoke`, and positional `claude-host-runner <socket>`
- [ ] 2.2 Wire CLI parsing in `main.rs` before delegating to `runtime::main_entry()`
- [ ] 2.3 Implement `--config-check` by instantiating `Config` and validating workspace roots exist/canonicalize
- [ ] 2.4 Implement `--doctor-smoke` by importing the existing doctor engine and printing JSON to stdout
- [ ] 2.5 Add integration tests in `tests/stdio_binary.rs` asserting `--version` and `--help` output

## 3. Configuration Integration

- [ ] 3.1 Replace ad-hoc `env::var("AGENT_BRIDGE_WORKSPACES")` in `claude_host.rs` with `Config::workspaces()`
- [ ] 3.2 Replace ad-hoc `env::var("AGENT_BRIDGE_STATE_DIR")` in `task.rs` with `Config::state_dir()`
- [ ] 3.3 Replace `env::var("AGENT_BRIDGE_MAX_ACTIVE_TASKS")` in `task.rs` with `Config::max_active_tasks()`
- [ ] 3.4 Ensure `safe_cwd` in `server.rs` uses the centralized `Config` workspace roots
- [ ] 3.5 Add unit tests for `Config` precedence (file < env < CLI) and home-directory expansion

## 4. Streaming Observation

- [ ] 4.1 Add `tokio::sync::watch` transmitter to `TaskActor` keyed by `agent_id`
- [ ] 4.2 Signal the watch channel from `drain_log` after flushing each batch of transcript lines
- [ ] 4.3 Signal the watch channel from `complete()` after finalization and transcript flush
- [ ] 4.4 Replace the 50 ms sleep loop in `observe()` with an `await` on the watch receiver, falling back to a bounded timeout
- [ ] 4.5 Add unit/integration test simulating a delayed producer and asserting the observer awakens promptly

## 5. Structured Logging

- [ ] 5.1 Add `tracing` and `tracing-subscriber` dependencies to `Cargo.toml`
- [ ] 5.2 Initialize JSON-formatted `tracing_subscriber` in `runtime::main_entry()` writing to stderr
- [ ] 5.3 Add `#![deny(clippy::print_stdout)]` attribute to `lib.rs` or equivalent lint configuration
- [ ] 5.4 Migrate `eprintln!("[agent-bridge] ...")` in `runtime.rs` to `tracing::error!` or `tracing::warn!`
- [ ] 5.5 Add `#[instrument(fields(agent_id, provider, mode))]` to `TaskActor::spawn`, `launch_task`, `wait_for_child`, and `complete`
- [ ] 5.6 Verify with `grep -r 'println!' src/` and `grep -r 'eprintln!' src/` that residual raw prints are eliminated
- [ ] 5.7 Run `scripts/quality.sh` to ensure the new tracing code passes clippy and fmt

## 6. Hot Reload

- [ ] 6.1 Write a PID lock file (`state_dir/server.pid`) on server startup, removing it on graceful shutdown
- [ ] 6.2 Implement `reload` CLI subcommand that reads the lock file and sends `SIGHUP` to the server PID
- [ ] 6.3 Handle `SIGHUP` in `shutdown_signal()` by reloading `Config` and re-canonicalizing workspace roots
- [ ] 6.4 Ensure reload errors preserve the incumbent workspace set and emit a `tracing::error!` event
- [ ] 6.5 Add integration test asserting reload behavior: initial config, mutation, signal, and updated rejection/acceptance of `cwd`

## 7. Regression & Polish

- [ ] 7.1 Run full test suite: `cargo test -- --test-threads=1`
- [ ] 7.2 Run hard gates: `scripts/quality.sh`
- [ ] 7.3 Update `docs/agents/` or `README.md` with new CLI and config file documentation
- [ ] 7.4 Archive the change via `openspec archive` when all tasks are ticked
