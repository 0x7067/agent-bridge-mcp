# Dogfood Provider/Profile Comparison

Use this workflow to run the same read-only prompt through one or more providers
with both launch profiles: `bridge` and `bare`. The harness talks to the local
Agent Bridge MCP server over stdio, waits for finality, and writes evidence for
each provider/profile pair.

`unblocked` is intentionally outside this default read-only comparison harness.
Use it only for explicit workspace-permission reach checks after reviewing the
dry-run command and profile diagnostics.

## What It Captures

For each run, the harness writes:

```text
artifacts/dogfood/<timestamp>/
├── manifest.json
├── server_stderr.log
└── runs/
    └── <provider>/
        ├── bridge/
        │   ├── agent_spawn.json
        │   ├── agent_observe.json
        │   ├── task_result.json
        │   └── task_transcript.json
        └── bare/
            ├── agent_spawn.json
            ├── agent_observe.json
            ├── task_result.json
            └── task_transcript.json
```

`task_transcript.json` is copied from `agent_result` with
`sections: ["transcript"]`. `task_result.json` is the full `agent_result`
evidence payload with `sections: ["summary", "stdout", "stderr", "transcript"]`.

Provider output is evidence, not proof. Use the artifacts to compare provider
behavior, then verify conclusions in the main caller.

## Run It

Build the stdio server:

```bash
rtk cargo build --bin agent-bridge-mcp
```

Run the default checked-in prompt against Codex with both profiles:

```bash
rtk python3 scripts/dogfood_compare.py --providers codex
```

Compare multiple providers:

```bash
rtk python3 scripts/dogfood_compare.py --providers codex,cursor,kimi
```

Preview the strict-validation spawn matrix without launching providers:

```bash
rtk python3 scripts/dogfood_compare.py --providers codex,cursor,kimi --strict-validation --dry-run
```

Dry-run preflight writes `manifest.json` and each `agent_spawn.json` preview,
then stops before `agent_observe` and `agent_result`. If a provider/profile
cannot be previewed, the harness records that run as `status: "failed"` with
`error.json` and continues the rest of the matrix. It does not launch providers,
but it still validates provider configuration such as required binary
environment variables.

Bake in strict provider output validation before flipping the default:

```bash
rtk python3 scripts/dogfood_compare.py --providers codex,cursor,kimi --strict-validation --require-success
```

Use a custom prompt or output directory:

```bash
rtk python3 scripts/dogfood_compare.py \
  --providers codex,cursor \
  --prompt-file examples/dogfood/read-only-prompt.md \
  --output-dir artifacts/dogfood/local-comparison
```

The harness defaults `AGENT_BRIDGE_WORKSPACES` to the selected `--cwd` when the
environment does not already set it. If provider readiness is uncertain, use an
MCP client to call `doctor` with `focus: "providers"` and `smoke: true` before
the comparison.

For Claude, start the host runner and export `AGENT_BRIDGE_CLAUDE_HOST_SOCKET`
before running the harness.

## Interpreting Results

Open `manifest.json` first. It lists each provider/profile run, the `agentId`,
final status, and paths to the captured transcript and result evidence.
Dry-run summaries use `status: "preview"` and `spawnPath` instead of
`agentId`, `resultPath`, and `transcriptPath`.
Failed preview summaries use `status: "failed"` and `errorPath`.

Compare these fields across `bridge` and `bare`:

- `task_result.json` `reviewPacket.profile`
- `task_result.json` `reviewPacket.profileDiagnostics`
- `task_result.json` `stdout` and `stderr`
- `task_transcript.json` `events`
- final provider prose in `task_result.json`

The prompt is read-only and `agent_spawn` uses `isolation: "none"`, so the run is
intended for behavior comparison rather than implementation. If a provider
changes files anyway, treat that as a provider/profile finding and inspect the
workspace before continuing.
