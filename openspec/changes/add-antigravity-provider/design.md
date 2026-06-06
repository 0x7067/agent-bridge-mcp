## Overview
Add an `antigravity` provider adapter that treats Google Antigravity CLI (`agy`) as a direct non-interactive provider. Local evidence from `agy --help` shows `--print`/`-p`, `--prompt`, `--prompt-interactive`, `--print-timeout`, `--model`, `--sandbox`, `--dangerously-skip-permissions`, `--add-dir`, and conversation resume flags. Official docs describe `agy` configuration in `~/.gemini/antigravity-cli/settings.json`, `--sandbox`/`--dangerously-skip-permissions` launch overrides, and authentication via OS keyring or browser OAuth.

## Command Shape
Version probe:

```text
agy --version
```

Task and smoke execution:

```text
agy --print <rendered-prompt> --print-timeout <timeout>s [--model <model>] [--sandbox]
```

Antigravity print mode exits after a single prompt and prints the response. `--print-timeout` should track the Agent Bridge task timeout so process-level timeout and provider-level timeout align.

## Modes And Safety
Support all existing task modes: `research`, `review`, `implement`, and `command`.

For `research` and `review`, pass `--sandbox` because it is the only verified CLI-level restriction flag, but do not claim it proves read-only file behavior. Until an implementation spike proves stronger filesystem semantics, read-only behavior remains prompt-enforced by the Agent Bridge task wrapper plus Antigravity's own permission flow. For `implement` and `command`, do not auto-pass `--dangerously-skip-permissions`; Antigravity should keep its normal permission behavior unless the user has configured it outside Agent Bridge. Agent Bridge already enforces workspace roots and process timeouts.

The implementation must include a bounded write-safety spike using a fake or disposable workspace if live Antigravity credentials are available. If credentials are unavailable, document that read-only mode is not verified live and keep capability metadata honest.

## Profiles
Support `bridge` and `bare`.

- `bridge`: normal Agent Bridge prompt wrapper.
- `bare`: compact prompt only. Antigravity docs and local help do not expose reliable flags to disable skills, hooks, memory/session state, context files, or ambient settings in print mode. Report those reductions as unsupported or best-effort instead of claiming isolation.

## Environment Policy
Use the existing non-Claude environment allowlist plus Antigravity-specific entries:

- `AGY_BIN` for binary override.

The selected command path should resolve from `AGY_BIN` or fall back to `agy`. Do not add raw credential env vars unless implementation evidence proves Antigravity print mode needs them; official docs describe OS keyring/browser auth rather than a required environment variable path. If env-clearing prevents a supported auth path, report that limitation through readiness diagnostics instead of broadening the allowlist preemptively.

## Readiness
Version-only readiness behaves like other providers: available but not launchable. Smoke readiness runs the same print-mode path with the minimal smoke prompt and accepts stdout containing `AGENT_BRIDGE_PROVIDER_SMOKE_OK`.

If `agy --print` asks for authentication and times out, the smoke result should preserve binary availability from the successful version probe while setting `startupVerified: false`, `launchable: false`, and a bounded auth/smoke diagnostic. The provider remains statically listed.

## Tests
- Unit/protocol tests for provider enum/schema/capability metadata including `antigravity`.
- Preview tests proving Antigravity command shape, prompt redaction, model flag, timeout flag, sandbox flag placement, and bare-profile diagnostics.
- Stdio fake-provider tests proving `AGY_BIN` is used for version and smoke checks, provider filters accept `antigravity`, providerTimeoutMs keys include `antigravity`, and auth-required print mode preserves version availability while failing startup readiness.
- Doctor tests proving provider filters/timeouts accept `antigravity` and environment diagnostics include `AGY_BIN`.
- Existing full test suite remains fake-provider based and must not require live Antigravity auth.

## Verification
Run:

```text
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release --bin agent-bridge-mcp
install -m 0755 target/release/agent-bridge-mcp ~/.local/bin/agent-bridge-mcp
cmp -s target/release/agent-bridge-mcp ~/.local/bin/agent-bridge-mcp
```

After copying the binary, smoke the installed MCP tool surface sequentially so it cannot race the old installed binary.
