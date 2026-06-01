## Context

The Claude provider currently launches `claude-p` as a child of the MCP server. That works for deterministic fake providers, but it fails for real Claude Code auth when the MCP server runs under Codex's macOS sandbox: Claude Code's logged-in state is stored in macOS Keychain and is not available to the sandboxed child process.

Live checks isolated the failure:
- `claude-p` inside the Codex/MCP sandbox timed out or saw Claude auth as unavailable.
- `claude-p` outside the sandbox succeeded.
- The bridge's existing `/bin/zsh -flc` wrapper outside the sandbox also succeeded.

That means the shell wrapper and `claude-p` itself are not the primary problem. The missing boundary is a deliberate host-side execution path for Claude.

## Goals / Non-Goals

**Goals:**
- Run Claude provider task and smoke commands through `claude-p` outside the Codex sandbox when configured.
- Keep the host escape narrow: Claude provider only, local Unix socket only, workspace/cwd validation preserved, no arbitrary command execution.
- Preserve the public MCP lifecycle API and diagnostics shape.
- Keep deterministic fake-provider tests possible without host Keychain or live Claude access.
- Document the operator setup and failure modes.

**Non-Goals:**
- Do not use `clm`.
- Do not switch the default solution to native `claude -p`.
- Do not enable Codex remote-control or depend on Codex app-server `process/spawn`.
- Do not make all providers run outside the sandbox.
- Do not silently fall back from host-runner execution to sandboxed Claude when host execution is explicitly required.

## Decisions

### Decision 1: Add a purpose-built Claude host runner instead of broad remote control

Implement a second bridge-owned executable mode that listens on a local Unix socket and runs only validated Claude provider commands. The MCP process talks to that socket when `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` is configured.

Alternative considered: Codex app-server `process/spawn`. It is documented as outside-sandbox process execution, but it requires enabling a broader experimental host-control surface. This change needs only `claude-p`, so a narrow runner is the smaller trust boundary.

Alternative considered: environment passthrough. Passing `ANTHROPIC_AUTH_TOKEN` or `CLAUDE_CODE_OAUTH_TOKEN` can help API-token setups, but it does not solve the normal logged-in Claude Code path where auth is stored in macOS Keychain.

### Decision 2: Make host-runner use explicit and visible

Host-runner execution is opt-in through an explicit socket path. `task_preview`, `providers_check`, and task diagnostics should report that Claude uses a host runner when configured.

If a host runner is configured but unavailable, Claude smoke checks and tasks fail with an actionable `host_runner_unavailable` diagnostic rather than falling back to sandboxed execution. Silent fallback would reintroduce the exact failure mode this change is meant to eliminate.

### Decision 3: Keep runner and MCP workspace policy synchronized

The host runner has its own startup configuration and must receive `AGENT_BRIDGE_WORKSPACES`. It fails startup if the workspace list is missing, empty, or contains any path that cannot be canonicalized. Each request also includes the MCP server's workspace list fingerprint; the runner rejects requests whose fingerprint does not match its startup configuration.

This keeps the runner fail-closed if Codex starts the MCP with a different workspace policy than the already-running host process.

Alternative considered: trust the MCP server's prior cwd validation only. That would be simpler but weakens the host boundary because a compromised or misconfigured sandboxed MCP could ask the host runner to execute outside the intended workspace.

### Decision 4: Enforce socket permissions

The host runner creates or validates a user-owned socket directory with `0700` permissions before binding. It removes stale sockets only inside that directory, binds the Unix socket, and then sets socket file permissions to `0600` where the platform exposes filesystem permissions for sockets. If the directory cannot be made owner-only, runner startup fails.

Alternative considered: best-effort permission tightening. That is too weak for an explicit sandbox escape path.

### Decision 5: Use structured runner requests and reconstruct commands in the runner

The MCP does not send a shell command descriptor to the host runner. It sends a structured Claude request: protocol version, request type, workspace policy id, cwd, timeout, task mode, optional model, optional effort, and prompt stdin payload. The host runner reconstructs the exact `claude-p` invocation from a hardcoded bridge-owned template and its own `CLAUDE_P_BIN`/`PATH` environment.

This eliminates the need to validate shell strings or parse arbitrary argv. The runner supports only `claude-p`; native `claude -p` remains outside this host-runner change.

Alternative considered: send the existing `ProviderCommand` descriptor and validate that it matches the bridge wrapper. Kimi correctly flagged that as the wrong trust boundary because weak descriptor matching could become arbitrary command execution through `/bin/zsh -flc`.

### Decision 6: Use newline-delimited JSON framing

The host-runner protocol uses one JSON object per line over the Unix socket. Requests and responses include a protocol version field. The runner enforces a hard maximum request-line size before JSON parsing and rejects oversized or unterminated requests without continuing to buffer. This framing is simple to test with standard Rust async I/O and sufficient because provider stdout/stderr are encoded inside JSON strings, not streamed raw over the protocol.

Alternative considered: 4-byte length-prefix framing. That is more binary-protocol friendly but unnecessary for bounded request/response payloads and harder to inspect manually.

### Decision 7: Preserve direct execution descriptors for non-host paths

The Claude adapter will continue to build the existing `/bin/zsh -flc` command descriptor and stdin prompt transport for direct execution. For host-runner execution it will also attach structured Claude launch metadata, derived from the same validated task input.

This keeps command construction, redaction, prompt rendering, cwd handling, and timeout behavior in one path.

### Decision 8: Host runner validates before spawning

The host runner MUST reject requests unless all of these hold:
- Request type is a supported Claude runner request.
- Request protocol version matches the runner protocol.
- Task mode is one of the Claude-supported modes.
- The requested `cwd` canonicalizes under configured `AGENT_BRIDGE_WORKSPACES`.
- The request workspace policy id exactly matches the runner startup workspace policy id.
- Timeout is bounded by the existing task timeout policy.
- Model and effort values pass the same Claude provider validation used by the MCP.

The runner reconstructs the `claude-p` command internally and captures stdout/stderr with bounded streaming readers. Capture must never read an unbounded stream into memory and then truncate after the fact; it keeps at most the configured byte cap per stream plus small accounting metadata. The runner returns whether each stream was truncated, exit code or signal, elapsed time, and failure category. It never writes provider output to MCP stdout or its own logs.

The runner handles each accepted connection independently with a bounded Tokio task. The first implementation may run one process per request without a queue, but it must not block unrelated socket accepts while a Claude process is running.

While a child process is running, the runner also monitors the client socket for EOF. If the client disconnects before the child exits, the runner terminates the child process group where supported, falls back to terminating the direct child where not supported, and reaps the child.

On runner SIGTERM or SIGINT, the runner stops accepting new connections, terminates active child process groups where supported, falls back to terminating direct children where not supported, waits for children to exit, and then exits.

### Decision 9: Define workspace policy id deterministically

The workspace policy id is the sorted, canonicalized absolute workspace path list joined by a NUL byte. This is not a secret or authorization token; it is drift detection so a long-running host runner rejects requests from an MCP process started with a different workspace policy. The runner still enforces cwd canonicalization against its own configured workspace list.

### Decision 10: Define error and health protocol responses

Every host-runner response is either `ok: true` with a typed result or `ok: false` with an error object containing `code` and `message`. Error codes include `protocol_mismatch`, `invalid_request`, `workspace_policy_mismatch`, `cwd_outside_workspace`, `timeout`, and `spawn_failed`.

Runner-generated error messages are sanitized and must not include prompts, token values, raw env values, full cwd paths, workspace paths, socket paths, or command arguments. Runner stderr logs are even stricter: they emit only stable error codes, coarse categories, and elapsed timing. Detailed diagnostics flow through MCP-side redaction, not runner logs.

The protocol includes a `ping` request that returns runner version, protocol version, workspace policy id, and whether the runner is ready to accept Claude requests.

### Decision 11: Keep direct execution as the deterministic test path

When no host socket is configured, direct child-process execution remains available. Tests can also use fake host-runner fixtures to exercise host-launch behavior without requiring Keychain or live Claude.

## Risks / Trade-offs

- Host runner is an intentional sandbox escape -> Mitigation: make it Claude-only, local-socket-only, workspace-validated, command-validated, and opt-in.
- Runner lifecycle adds setup complexity -> Mitigation: ship a dedicated runner command and document a one-command smoke plus launchd-friendly setup.
- Socket permissions may be too permissive -> Mitigation: require a user-owned `0700` socket directory and `0600` socket file permissions where supported; fail startup when directory permissions cannot be tightened.
- Host runner and MCP versions can drift -> Mitigation: include a simple protocol version in requests and fail clearly on mismatch.
- Live verification can spend model quota -> Mitigation: keep live smoke opt-in and preserve fake-provider tests as the default CI path.
- Runner raw logs could expose provider output -> Mitigation: runner stderr logs are limited to lifecycle metadata and validation failures; raw provider stdout/stderr is returned to the MCP for existing redaction and diagnostics, not logged by the runner.
- Provider output can grow without bound -> Mitigation: use bounded streaming capture with a fixed per-stream memory cap, mark truncation in the response, and keep MCP diagnostics capped as they are today.
- Runner validation logs could expose paths -> Mitigation: runner stderr logs use only error codes and coarse categories; response messages are sanitized and MCP diagnostics perform any richer redacted reporting.
- Request input can grow without bound -> Mitigation: enforce a hard pre-parse request-line cap and reject oversized or unterminated requests.

## Migration Plan

1. Implement and test the host runner protocol with fake Claude providers.
2. Update README with setup, preview, smoke, and troubleshooting steps.
3. Build/install the updated binary.
4. Start the host runner outside Codex's sandbox.
5. Verify the runner socket with a protocol ping or Claude fake-provider smoke before reloading MCP configuration.
6. Reload MCP configuration and run a Claude-only live smoke check.

Rollback is to unset `AGENT_BRIDGE_CLAUDE_HOST_SOCKET` and stop the host runner; the existing direct execution path remains intact.
