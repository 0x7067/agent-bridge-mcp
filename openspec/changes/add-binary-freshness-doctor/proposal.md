## Why

Agent Bridge changes can pass tests while the installed `agent-bridge-mcp` binary remains stale. A diagnostic-only binary freshness report makes that mismatch visible through `doctor` before operators debug the wrong executable.

## What Changes

- Add `doctor.binary` diagnostics for the running executable, installed binary, and repo release binary candidate.
- Compare file existence, size, modified time, and a stable content fingerprint when files are readable.
- Classify freshness as `ok`, `warning`, `error`, or `unknown` without mutating or installing binaries.
- Add recommendations that tell operators to run the documented release build and install flow when the installed binary differs from the release candidate.
- Allow testable path overrides through environment variables rather than public MCP arguments.

## Capabilities

### New Capabilities

- `binary-freshness-doctor`: Covers read-only diagnostics for running, installed, and release Agent Bridge binaries, including freshness classification, path overrides, content comparison, and recommendations.

### Modified Capabilities

- `agent-bridge-doctor`: Doctor output must include additive binary freshness diagnostics and recommendations without changing delegated task verification or provider readiness semantics.

## Impact

- Affected code: doctor diagnostics and recommendation shaping in `crates/agent-bridge-mcp/src/server.rs`.
- Affected APIs: additive `binary` section in `doctor` responses.
- Affected docs/specs: README and guidance resources explaining binary freshness diagnostics and the build/install boundary.
- Dependencies: no new third-party dependency expected.
