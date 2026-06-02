## Codex Command Diagnosis

Agent Bridge and the installed MCP command both preview Codex implementation tasks as:

```text
codex exec --cd <repo-cwd> --json --sandbox workspace-write --config shell_environment_policy.inherit="all" <prompt>
```

The observed `task_preview` evidence does not implicate `--cd` or sandbox selection: `--cd` points at the requested repository cwd and implementation mode uses `workspace-write` as intended. Prompt transport remains argv-based for Codex, which is acceptable for command construction but means diagnostics must redact the rendered prompt when provider stderr echoes argv.

The root fix is lifecycle and diagnostics, not a Codex command-shape change:

- Detect Codex sandbox, approval, and out-of-workspace denial stderr.
- Finalize immediate-exit and hung-denial tasks as failed with `errorType: codex_sandbox_denied`.
- Keep the diagnostic category stable as `provider_sandbox_denied`.
- Terminate the spawned provider process group for bounded cleanup when a denial hangs.
- Preserve successful Codex-like fake-provider behavior.

The only Codex adapter correction in this change is redaction metadata for the rendered prompt and original task prompt, so diagnostics and `reviewPacket` do not leak prompt content if Codex or a wrapper echoes argv.

Post-implementation Bridge review flagged two runtime gaps that were fixed before final verification: standalone `patch rejected` evidence is now classified as a fatal denial, and the stderr polling path uses async log reads. The same review also flagged provider child-process cleanup, so normal task launches now create a child process group and denial/stop/shutdown termination targets that group.

## Bridge Observation

The read-only Codex Bridge observation task completed successfully without edits. It reported that command shape was not the likely cause and that current lifecycle handling was the implicated surface. It also exposed an environment issue in that spawned context: the installed `codex` wrapper uses `/usr/bin/env node`, and `node` was absent from that PATH. That issue is operationally relevant for live dogfood, but the default regression coverage remains fake-provider based and does not require live Codex.
