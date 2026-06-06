## Context
The public `agent_*` API intentionally rejects `taskId` request fields. That public break should remain. The failing path is different: persisted registry data from previous versions used `taskId` and `taskDir`, and the current loader treats that durable state as incompatible even when the file is valid JSON.

The earlier `simplify-agent-api-config` change explicitly accepted old registry startup failure. Running against the real default state shows that decision blocks Agent Bridge itself before provider launch.

## Decisions

### Decision 1: Normalize persisted records on read
When loading `registry.json`, the server will parse the JSON value, translate `taskId` to `agentId` and `taskDir` to `agentDir` only when canonical fields are absent, then deserialize the normalized value into the typed registry.

This avoids manual registry edits and preserves historical task directories. It also lets the next normal registry save write the current field names.

### Decision 2: Do not add public `taskId` aliases
Public lifecycle requests still require `agentId`. Existing strict argument validation and output-shape tests remain the public API guardrail.

Legacy persisted `task_...` values may be returned as `agentId` for historical records. That is less clean than generating fresh `agent_...` identifiers, but it avoids a risky implicit rename of directories, registry keys, worktree metadata, and caller-visible history.

### Decision 3: Reuse the typed parser from doctor
`doctor` should not only parse registry JSON syntax. It should validate the same normalized typed registry shape used by lifecycle startup, without starting the async task manager.

## Risks / Trade-offs
- Historical records can surface `task_...` strings in `agentId` fields. This is a compatibility concession for persisted state only, not a public request alias.
- A malformed legacy record can still fail to load if required non-identifier fields are missing. That should remain a clear registry diagnostic.
- The active `simplify-agent-api-config` delta contains the old no-migration decision until archived or superseded. This change supersedes that persisted-state behavior.

## Rollback
Revert the parser, doctor, tests, and this OpenSpec change. No on-disk registry mutation is required by this change, so rollback does not require state restoration.
