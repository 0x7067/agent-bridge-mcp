# Archived Dogfood Provider/Profile Comparison

This page documents a removed workflow. The legacy Python harness used the old
MCP lifecycle tools, so it was deleted when Agent Bridge moved to the ACP router
and the two-tool MCP adapter.

Use this page only when reading older artifacts. New provider checks should use
the ACP router directly or the `agent_delegate` / `agent_evidence` adapter path
after a new harness is built.

## Legacy Artifacts

For each run, the removed harness wrote:

```text
artifacts/dogfood/<timestamp>/
├── manifest.json
├── server_stderr.log
└── runs/
    └── <provider>/
        ├── bridge/
        │   ├── spawn-preview.json
        │   ├── observe-events.json
        │   ├── task_result.json
        │   └── task_transcript.json
        └── bare/
            ├── spawn-preview.json
            ├── observe-events.json
            ├── task_result.json
            └── task_transcript.json
```

`task_transcript.json` and `task_result.json` were copied from the legacy result
reader. Equivalent ACP-era evidence should be captured through `agent_evidence`.

Provider output is evidence, not proof. Use the artifacts to compare provider
behavior, then verify conclusions in the main caller.

## Current Replacement

For provider readiness, use the built-in smoke diagnostic:

```bash
agent-bridge-mcp --doctor-smoke --provider <name>
```

For end-to-end delegated work, use an ACP `session/prompt` turn or the MCP
adapter's `agent_delegate` tool, then fetch bounded evidence with
`agent_evidence`.

## Interpreting Results

Open `manifest.json` first. It lists each provider/profile run, the `agentId`,
final status, and paths to the captured transcript and result evidence.
Dry-run summaries used `status: "preview"` and `spawnPath` instead of
`agentId`, `resultPath`, and `transcriptPath`.
Failed preview summaries use `status: "failed"` and `errorPath`.

Compare these fields across `bridge` and `bare`:

- `task_result.json` `reviewPacket.profile`
- `task_result.json` `reviewPacket.profileDiagnostics`
- `task_result.json` `stdout` and `stderr`
- `task_transcript.json` `events`
- final provider prose in `task_result.json`

The removed prompt was read-only and the legacy spawn request used
`isolation: "none"`, so the run was intended for behavior comparison rather than
implementation. If an older artifact shows file changes anyway, treat that as a
provider/profile finding and inspect the workspace before continuing.
