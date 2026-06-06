## 1. Contract

- [x] 1.1 Review the plan through Agent Bridge or document the registry-load blocker and use an isolated Agent Bridge state fallback.
- [x] 1.2 Add an OpenSpec delta superseding legacy registry startup failure.

## 2. Implementation

- [x] 2.1 Add read-time registry normalization for legacy `taskId` and `taskDir` persisted fields.
- [x] 2.2 Keep public lifecycle input validation strict so `taskId` remains rejected.
- [x] 2.3 Make `doctor` validate registry state through the typed compatibility parser.

## 3. Verification

- [x] 3.1 Add focused unit coverage for legacy registry load.
- [x] 3.2 Add stdio coverage proving legacy records are inspectable through public `agentId` output without public `taskId` keys.
- [x] 3.3 Add or update doctor coverage for typed registry compatibility.
- [x] 3.4 Run OpenSpec validation, formatting, focused tests, full tests, clippy, release build, installed-binary replacement, and installed-binary smoke against the real state dir.
