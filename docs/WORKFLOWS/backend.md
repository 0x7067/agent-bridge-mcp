# Backend Workflows

**Last Updated:** 2026-06-20
**Based on patterns from:** `src/router_runtime.rs`, `src/mcp_adapter.rs`, `src/provider.rs`, `tests/stdio_binary.rs`, `tests/mcp_adapter_protocol.rs`

## How to Change the Public Protocol Surface

Agent Bridge is ACP-router-first. Keep new client-facing behavior in one of two
places:

- ACP default runtime: `src/router_runtime.rs` and `src/router.rs`
- MCP compatibility adapter: `src/mcp_adapter.rs`

The adapter intentionally exposes only `agent_delegate` and `agent_evidence`.
Do not add lifecycle tools back unless the product direction changes.

### ACP Router Changes

For routing behavior, update `router.rs` policy or `router_runtime.rs` request
handling. Tests usually belong in:

- `tests/stdio_binary.rs` for end-to-end stdio behavior.
- `tests/router_policy.rs` for policy-only behavior.

Run the focused target first:

```bash
cargo test -p agent-bridge-mcp --test stdio_binary -- --test-threads=1
```

### MCP Adapter Changes

For MCP compatibility behavior, update `mcp_adapter.rs`. Keep schemas strict
and bounded, and preserve the default `verificationStatus: "not_verified"`
contract.

Run the adapter tests:

```bash
cargo test -p agent-bridge-mcp --test mcp_adapter_protocol
```

### Internal Task Lifecycle Changes

Task state, spawn validation, evidence shaping, and cleanup live behind the
public surfaces in `task.rs` and `task/*`. Prefer changing the narrow submodule
that owns the behavior:

- `task/input.rs` for spawn input structures.
- `task/spawn.rs` for validation, worktree setup, and provider launch.
- `task/complete.rs` for exit/result classification.
- `task/review.rs` for summaries, next actions, and evidence payloads.
- `task/registry.rs` for persisted state.

## How to Add a New Provider Adapter

### Step 1: Declare Support in Capabilities

Edit `provider.rs` capabilities and add a new provider entry:

```rust
"new_provider": {
    "modes": ["research", "review"],
    "supportsReply": false,
    "supportsResume": false,
    "supportsWorktreeIsolation": true,
    "launchProfiles": ["bridge", "bare"], // add "unblocked" only when the adapter has known permission-bypass args
    "readiness": default_readiness()
}
```

### Step 2: Wire the Provider Enum

Add `NewProvider` to `ProviderKind` in `domain.rs` and derive the `as_str()` mapping. Also update `provider_names()`.

### Step 3: Implement Command Construction

Extend `provider.rs` `prepare_command()` (or equivalent) to recognize the new provider and build its `ProviderCommand`:

```rust
ProviderKind::NewProvider => {
    ProviderCommand {
        provider: ProviderKind::NewProvider,
        command: "new-provider-cli".to_string(),
        args: vec!["exec".to_string(), "--prompt".to_string(), prompt.to_string()],
        stdin: None,
        redactions: vec![],
        cwd: cwd.to_string(),
        timeout_seconds,
        env: provider_env(ProviderKind::NewProvider),
        profile,
        prompt_strategy: "inline_arg".to_string(),
        profile_diagnostics: json!({}),
        ..Default::default()
    }
}
```

### Step 4: Add Denial Detection (Optional)

If the provider emits recognizable stderr on failure, add an adapter method:

```rust
fn detects_fatal_denial(&self, stderr: &[u8]) -> bool {
    String::from_utf8_lossy(stderr).contains("SANDBOX_DENIED")
}
```

Register the adapter in `adapter_for()`.

### Step 5: Add Fixtures and Smoke Tests

Provide a fake script under `tests/fixtures/` and wire it into provider smoke or
router/adapter protocol tests as appropriate.

## Running Quality Gates Locally

```bash
./scripts/quality.sh
```

Hard gates (script exits non-zero on failure):
- `cargo fmt --all --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo machete`
- `npx jscpd`

Informational only:
- Complexity hotspot warnings
- Module dependency graph (acyclic + boundary review)

Fix any hard gate violation before opening a PR.
