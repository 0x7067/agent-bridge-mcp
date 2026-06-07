## Archive Follow-up

When archiving this change, merge the delta requirements into the base specs rather than leaving them as guidance-only notes.

Pay particular attention to:

- `agent-bridge-self-guidance`: update the running-agent next-action wording so `agent_observe` is the primary running-agent recommendation, with `agent_wait` and raw inspection tools as follow-up diagnostics.
- `mcp-usage-guidance`: keep the compact primary workflow separate from diagnostic/recovery tool guidance.
- `agent-bridge-agent-presentation`: preserve the primary ordering of observe/result before lower-level diagnostics and cleanup.
- `rust-single-binary-mcp`: preserve the current callable tool surface and no legacy `task_*` lifecycle tools.
