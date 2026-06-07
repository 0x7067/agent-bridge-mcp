## Context

The release binary is built from this repo and installed manually to `~/.local/bin/agent-bridge-mcp`. After runtime changes, operators need a quick way to confirm which binary is running and whether the installed binary matches the latest release build artifact.

## Goals / Non-Goals

**Goals:**

- Report the running executable path from `std::env::current_exe`.
- Report the installed binary path, defaulting to `~/.local/bin/agent-bridge-mcp`.
- Report a release candidate path, defaulting to `<doctor cwd or process cwd>/target/release/agent-bridge-mcp`.
- Compare readable files with size, modified time, and a stable content fingerprint.
- Keep diagnostics read-only and avoid new dependencies.

**Non-Goals:**

- Do not build, install, copy, delete, or mutate binaries from `doctor`.
- Do not claim project tests or delegated provider output are verified.
- Do not require cryptographic hashing.
- Do not make binary freshness a provider launch-readiness condition.

## Decisions

### Decision: Add `doctor.binary`

Binary freshness belongs in `doctor` because it is setup/readiness evidence. It should not be a new lifecycle tool and should not affect provider readiness.

`doctor.binary` shape:

```json
{
  "status": "warning",
  "fingerprintLimitBytes": 16777216,
  "running": {
    "path": "/Users/pedro/.local/bin/agent-bridge-mcp",
    "exists": true,
    "readable": true,
    "sizeBytes": 3200000,
    "modifiedAt": "2026-06-05T10:00:00.000Z",
    "fingerprint": "fnv64:...",
    "fingerprintStatus": "ok",
    "error": null
  },
  "installed": {
    "path": "/Users/pedro/.local/bin/agent-bridge-mcp",
    "exists": true,
    "readable": true,
    "sizeBytes": 3200000,
    "modifiedAt": "2026-06-05T10:00:00.000Z",
    "fingerprint": "fnv64:...",
    "fingerprintStatus": "ok",
    "matchesRunning": true,
    "matchesRelease": false
  },
  "release": {
    "path": "/Users/pedro/Development/agent-bridge-mcp/target/release/agent-bridge-mcp",
    "exists": true,
    "readable": true,
    "sizeBytes": 3201000,
    "modifiedAt": "2026-06-05T10:05:00.000Z",
    "fingerprint": "fnv64:...",
    "fingerprintStatus": "ok",
    "matchesRunning": false
  },
  "recommendations": []
}
```

Each target uses the shared metadata shape: `path`, `exists`, `readable`, `sizeBytes`, `modifiedAt`, `fingerprint`, `fingerprintStatus`, and optional `error`. `installed` adds `matchesRelease` and `matchesRunning`; `release` adds `matchesRunning`.

Binary recommendations use both surfaces:

- `binary.recommendations`: concise strings for local section rendering.
- top-level `doctor.recommendations`: structured entries ordered after setup/provider/client recommendations. Build/install follow-ups use `kind: "shell"` and `command` arrays such as `["cargo", "build", "--release", "--bin", "agent-bridge-mcp"]` or `["install", "-m", "0755", "target/release/agent-bridge-mcp", "~/.local/bin/agent-bridge-mcp"]`.

Status rules:

- `ok`: installed and release are readable and match.
- `warning`: installed/release differ, installed missing, release missing, running differs from installed, or fingerprint skipped because a file exceeds the limit.
- `error`: current executable cannot be resolved or a configured override path is unreadable for reasons other than missing.
- `unknown`: insufficient metadata exists to compare freshness and no stronger warning/error applies.

### Decision: Environment overrides for testability

Use `AGENT_BRIDGE_INSTALLED_BIN` and `AGENT_BRIDGE_RELEASE_BIN` as optional path overrides. These are process environment inputs, not public MCP arguments, so tests can provide fixtures without expanding the public API.

### Decision: Stable non-cryptographic fingerprint

Use a simple FNV-1a 64-bit fingerprint over file contents up to a 16 MiB file-size cap. It is deterministic, standard-library only, and enough to compare two local files in diagnostics. The field name should be `fingerprint`, not `sha` or `checksum`, to avoid overclaiming cryptographic strength. If a file is larger than 16 MiB, set `fingerprintStatus: "skipped_too_large"` and do not read it.

### Decision: Release path resolution

Use `AGENT_BRIDGE_RELEASE_BIN` when set. Otherwise, if `doctor.cwd` is provided, use `<doctor cwd>/target/release/agent-bridge-mcp`; if no `cwd` is provided, use `<process cwd>/target/release/agent-bridge-mcp`.

### Decision: Summary status stays separate

Binary freshness issues appear in `doctor.binary.status` and recommendations. They do not change `summary.status`, because the running server can still be healthy while an installed or release candidate path is absent or stale.

## Risks / Trade-offs

- Fingerprint is not cryptographic -> Name it as a diagnostic fingerprint and compare it only locally.
- Release path may not exist outside this repo -> Report `unknown` or `warning` without failing doctor.
- Running executable can be a test binary or temporary path -> Report it as evidence without assuming it is the installed binary.
- Reading large binaries costs time -> Cap fingerprint reads at a bounded file size or skip unreadably large files with a warning.
