## Implementation Evidence

Local Antigravity CLI evidence:

- `agy --help` exposes non-interactive `--print`, `--print-timeout`, `--model`, `--sandbox`, and `--dangerously-skip-permissions` flags.
- Installed-binary version-only `providers_check` reports Antigravity available with version `1.0.6`.
- Bounded installed-binary smoke with `providerTimeoutMs.antigravity=3000` reports `available: true`, `startupVerified: false`, `launchable: false`, `readiness.state: "failed"`, and smoke-phase diagnostics because local Antigravity auth is not available to print mode.

Read-only safety:

- The implementation passes `--sandbox` for Antigravity `research` and `review`.
- Live write-safety is not verified because local Antigravity print mode requires authentication before a disposable workspace probe can run.
- Runtime metadata, README, and guidance therefore describe Antigravity non-mutating modes as prompt-enforced rather than verified read-only filesystem enforcement.
