## Claude Hook Relay Contract

Runner-owned Claude hooks use a private append-only event-log relay. Hook stdout
is not the data channel because Claude Code injects hook stdout into the
conversation for some events.

## Temporary Directory

For each run, the host runner creates a temporary directory with owner-only
permissions (`0700`). The directory contains:

- `settings.json`: runner-owned temporary Claude settings (`0600`);
- `hook-relay`: executable helper script or binary (`0700`);
- `events.log`: hook relay event log (`0600`);
- optional bounded diagnostic files owned by the runner.

The runner removes these artifacts after success, failure, timeout, disconnect,
or shutdown where cleanup is possible.

## Environment

The runner passes these environment variables only to the Claude child and hook
helper:

| Name | Meaning |
| --- | --- |
| `AGENT_BRIDGE_CLAUDE_HOOK_EVENT_LOG` | Absolute path to the per-run event log. |
| `AGENT_BRIDGE_CLAUDE_RUN_ID` | Opaque per-run id for correlation. |

The old upstream `CLAUDE_P_FIFO` name is not part of the Agent Bridge contract.

## Settings

The temporary settings JSON registers runner-owned hooks for:

- `SessionStart`
- `Stop`
- `StopFailure`

The settings are supplied through `--settings` and do not edit durable
`~/.claude`, project, or local settings files.

## Event Format

Each hook helper appends one line to the event log:

```text
<event-name>\t<payload-json>\n
```

`event-name` must be one of `SessionStart`, `Stop`, or `StopFailure`. The JSON
payload is the hook stdin payload relayed unchanged except for transport
framing. The runner owns parsing, validation, bounding, and redaction.

## Ordering

Before spawning Claude, the runner:

1. Creates the temporary directory.
2. Creates the owner-only event log file.
3. Starts the event-log reader before spawning Claude so hook events are tailed
   from the beginning.
4. Writes temporary settings.
5. Spawns interactive Claude with the settings and relay environment.

The runner treats relay setup failure as `hook_relay_error` and does not spawn
Claude.

## Limits

- Single event line limit: 1 MiB.
- Total relay byte limit: 8 MiB per run.
- Malformed event lines are diagnostic failures when required for completion.
- Unexpected event names are ignored with bounded diagnostics unless they
  indicate protocol drift.

## Completion Rules

- `SessionStart` proves Claude startup hooks are active; it is not by itself
  input-ready.
- `SessionStart` supplies transcript metadata used for normal completion.
- `Stop` is the normal completion signal. The Stop helper writes a minimal event
  without reading stdin because current interactive Claude can leave Stop hook
  stdin open while waiting for the hook command to exit.
- `StopFailure` is a provider/API failure event and prevents success even when
  it includes `last_assistant_message`.
- If both `Stop` and `StopFailure` are observed, `StopFailure` wins unless
  source verification later proves Claude can legitimately emit both for one
  turn.
