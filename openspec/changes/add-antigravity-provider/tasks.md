## 1. Implementation
- [x] 1.1 Add `antigravity` to provider domain enums, schemas, filter validation, and static capability metadata.
- [x] 1.2 Implement Antigravity command construction, binary resolution via `AGY_BIN`, environment policy, profile diagnostics, mode validation, version checks, and smoke checks.
- [x] 1.3 Update task execution and readiness helpers so Antigravity smoke output is accepted and default smoke budgets include Antigravity.
- [x] 1.4 Update README and runtime guidance so users see Antigravity as a supported provider with honest auth/readiness caveats.
- [x] 1.5 Record the implementation evidence for Antigravity `--sandbox` write-safety; if live credentials are unavailable, document that non-mutating modes are prompt-enforced rather than verified read-only.

## 2. Tests
- [x] 2.1 Add protocol tests for provider capability metadata, tool schemas, and preview redaction/command shape.
- [x] 2.2 Add stdio fake-provider tests for `AGY_BIN`, provider filters, provider timeout keys, version checks, smoke checks, and auth-required smoke diagnostics.
- [x] 2.3 Add doctor coverage for `antigravity` filters/timeouts and `AGY_BIN` environment reporting.
- [x] 2.4 Run formatting, tests, clippy, release build, installed-binary copy/compare, and installed MCP smoke sequentially.
