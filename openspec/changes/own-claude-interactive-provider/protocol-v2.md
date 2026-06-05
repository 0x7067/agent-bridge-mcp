## Claude Host Runner Protocol V2

Protocol v2 replaces the legacy structured `claude-p` host-runner contract with
an owned interactive Claude runner contract. The MCP binary and host runner must
use the same protocol version; mismatches are failures, not compatibility
fallbacks.

## Request

Requests are JSON objects sent over the existing local Unix socket framing.

Required fields:

| Field | Type | Notes |
| --- | --- | --- |
| `version` | integer | Must be `2`. |
| `requestType` | string | Must be `claude_interactive`. |
| `workspacePolicyId` | string | Must match a configured host-runner workspace policy. |
| `cwd` | string | Absolute path validated by the workspace policy. |
| `mode` | string | One of `research`, `review`, `command`, `implement`. |
| `prompt` | string | Rendered Agent Bridge prompt. Sent to PTY input only; redacted from logs and diagnostics. |
| `timeoutSeconds` | integer | Positive task timeout bounded by host-runner policy. |

Optional fields:

| Field | Type | Notes |
| --- | --- | --- |
| `model` | string | Forwarded as `--model` after validation. No fallback model behavior. |
| `effort` | string | One of `low`, `medium`, `high`, `xhigh`, `max`. |
| `bareProfile` | boolean | Requests reduced inherited Claude settings while preserving runner-owned hooks. |
| `smokeToken` | string | Present only for provider smoke checks. |

Rejected fields:

- `command`
- `shell`
- `script`
- `argv`
- `executablePath`
- any field that asks the host runner to execute a caller-supplied command
  descriptor

If rejected fields are present, the host runner returns
`failureCategory: "protocol_rejected"` without spawning a process.

## Response

Responses are JSON objects with `version: 2` and
`responseType: "claude_interactive_result"`.

Required fields:

| Field | Type | Notes |
| --- | --- | --- |
| `version` | integer | Must be `2`. |
| `responseType` | string | Must be `claude_interactive_result`. |
| `status` | string | `success` or `failure`. |
| `durationMs` | integer | End-to-end runner duration. |
| `failureCategory` | string or null | Null only when `status` is `success`. |
| `exitCode` | integer or null | Child exit code when available. |
| `signal` | string or null | Terminating signal when available. |
| `ptyOutputExcerpt` | string | Bounded, ANSI-stripped excerpt for diagnostics. Prompt text redacted. |
| `ptyOutputTruncated` | boolean | True when excerpt was truncated. |
| `redactionsApplied` | array of strings | Redaction classes applied to diagnostic fields. |

Successful responses include:

| Field | Type | Notes |
| --- | --- | --- |
| `result.finalText` | string | Final assistant text used by `agent_result`. |
| `result.source` | string | `transcript` or `stop_last_assistant_message`. |
| `result.sessionId` | string or null | Claude session id when available. |

Hook fields are bounded metadata, never raw unbounded hook payloads:

| Field | Type | Notes |
| --- | --- | --- |
| `stop` | object or null | Stop event metadata, including bounded `transcriptPath` status and `lastAssistantMessagePresent`. |
| `stopFailure` | object or null | StopFailure metadata, including canonical `error`, bounded details, and mapped category. |
| `transcript` | object | Parse status, retry count, accepted/rejected path status, fallback use, and bounded diagnostics. |

Known `failureCategory` values for v2:

- `protocol_mismatch`
- `protocol_rejected`
- `host_runner_unavailable`
- `workspace_policy_rejected`
- `claude_setup_required`
- `claude_auth_error`
- `claude_billing_error`
- `claude_rate_limit`
- `claude_model_unavailable`
- `claude_api_error`
- `startup_timeout`
- `runner_timeout`
- `child_exit_error`
- `transcript_error`
- `hook_relay_error`
- `provider_output_error`

## Version Mismatch

The host runner rejects request versions other than `2` with
`protocol_mismatch` and no process spawn.

The MCP side rejects response versions other than `2` with
`protocol_mismatch` and does not attempt legacy print-mode parsing.

## Limits

Implementation constants should live with the protocol module and tests.
Initial limits:

- PTY diagnostic excerpt: 64 KiB after redaction.
- Single hook relay line: 1 MiB before truncation/rejection.
- Total hook relay bytes per run: 8 MiB.
- Transcript parse retry budget: 2 seconds total.
- Startup input-ready budget: 30 seconds for tasks and 60 seconds for smoke.
