## Claude StopFailure and Setup Signature Notes

Verified against:

- Official hooks docs: `https://code.claude.com/docs/en/hooks`
- Official CLI docs: `https://code.claude.com/docs/en/cli-usage`
- Installed CLI: `/Users/pedro/.local/bin/claude`
- Installed version: `2.1.165 (Claude Code)`
- Installed native build: `/Users/pedro/.local/share/claude/versions/2.1.165`

No live model prompt was run. Source behavior was inspected with bounded
`strings` searches against the installed native build and should be treated as
signature evidence, not a stable public API.

## StopFailure Hook Shape

Official docs say `StopFailure` runs instead of `Stop` when a turn ends due to
an API error. Hook output and exit code are ignored. The input extends common
hook fields with:

- `error`: matcher/filter error type;
- `error_details`: optional additional details;
- `last_assistant_message`: optional rendered error text shown in the
  conversation, not a successful assistant response.

Documented and installed-build-confirmed `error` values:

- `rate_limit`
- `authentication_failed`
- `oauth_org_not_allowed`
- `billing_error`
- `invalid_request`
- `model_not_found`
- `server_error`
- `max_output_tokens`
- `unknown`

The installed binary string table contains all of the above values.

## Mapping Targets

| StopFailure `error` | Agent Bridge category | Notes |
| --- | --- | --- |
| `authentication_failed` | `claude_auth_error` | Covers expired/invalid credentials, missing login, invalid API key, OAuth refresh failures, and Keychain/auth access issues. |
| `oauth_org_not_allowed` | `claude_auth_error` | Authenticated account exists but is not permitted by org policy. Diagnostic should mention permitted organization/login. |
| `billing_error` | `claude_billing_error` | Covers subscription/API-credit/billing access failures. |
| `rate_limit` | `claude_rate_limit` | Preserve `error_details` such as HTTP 429 when present. |
| `model_not_found` | `claude_model_unavailable` | Usually model name or entitlement issue. |
| `server_error` | `claude_api_error` | Claude/API-side failure. |
| `invalid_request` | `claude_api_error` | Request/schema/tool-call/conversation-state failure; include bounded details. |
| `max_output_tokens` | `claude_api_error` | Provider did not complete because output budget was exhausted. |
| `unknown` | `claude_api_error` | Preserve bounded details and raw error type. |

Unknown future `error` strings should map to `claude_api_error` with the raw
error string included in bounded metadata.

## Fixture Payloads

Use these fixture shapes before parser implementation:

```json
{"session_id":"fixture-session","transcript_path":"/tmp/claude/transcript.jsonl","cwd":"/repo","hook_event_name":"StopFailure","error":"rate_limit","error_details":"429 Too Many Requests","last_assistant_message":"API Error: Rate limit reached"}
```

```json
{"session_id":"fixture-session","transcript_path":"/tmp/claude/transcript.jsonl","cwd":"/repo","hook_event_name":"StopFailure","error":"authentication_failed","error_details":"OAuth refresh token is no longer valid","last_assistant_message":"Session expired. Please run /login to sign in again."}
```

```json
{"session_id":"fixture-session","transcript_path":"/tmp/claude/transcript.jsonl","cwd":"/repo","hook_event_name":"StopFailure","error":"oauth_org_not_allowed","error_details":"organization is not permitted","last_assistant_message":"Please log in with a permitted organization: claude auth login"}
```

```json
{"session_id":"fixture-session","transcript_path":"/tmp/claude/transcript.jsonl","cwd":"/repo","hook_event_name":"StopFailure","error":"billing_error","error_details":"subscription or credit access unavailable","last_assistant_message":"Your account does not have access to Claude. Please login again or contact your administrator."}
```

```json
{"session_id":"fixture-session","transcript_path":"/tmp/claude/transcript.jsonl","cwd":"/repo","hook_event_name":"StopFailure","error":"model_not_found","error_details":"model alias unavailable","last_assistant_message":"API Error: model not found"}
```

```json
{"session_id":"fixture-session","transcript_path":"/tmp/claude/transcript.jsonl","cwd":"/repo","hook_event_name":"StopFailure","error":"future_new_error","error_details":"future schema value","last_assistant_message":"API Error: future error"}
```

## Setup and First-Run Prompt Signatures

The owned runner should classify startup as setup-blocked when the PTY output
shows auth/setup/trust UI instead of reaching input-ready quiescence. The match
set should be case-insensitive, tolerate ANSI/control sequences, and operate on
bounded stripped PTY excerpts.

Auth/login signatures observed in installed binary strings:

- `Please run /login`
- `Session expired. Please run /login to sign in again.`
- `OAuth refresh token is no longer valid; run /login to re-authenticate`
- `Not logged in. Run claude auth login to authenticate.`
- `Please log in with a permitted organization: claude auth login`
- `Your account does not have access to Claude. Please login again or contact your administrator.`
- `with a Claude.ai account. API key authentication is not sufficient. Please run /login`
- `Your organization has disabled Claude subscription access for Claude Code`
- `Remove the credential and run: claude auth login`
- `forceLoginMethod is 'gateway' in managed settings; run interactive /login to authenticate.`

API/billing/rate signatures observed or documented:

- `API Error: Rate limit reached`
- `API Error: Request was aborted.`
- `API error (status`
- `invalid api key`
- `credit balance`
- `billing`
- `quota`
- `rate limit`

Trust/setup prompt signatures from upstream `claude-p` behavior and installed
binary strings:

- combined `trust` and `folder` in stripped terminal output;
- `workspace trust` / `workspace trusted` style tokens;
- `Security: apiKeyHelper executed before workspace trust is confirmed`;
- `Security: awsAuthRefresh executed before workspace trust is confirmed`;
- `Security: gcpAuthRefresh executed before workspace trust is confirmed`.

## Detection Rules Before Implementation

- Prefer structured `StopFailure` hook payloads over PTY string matching for API
  failures.
- Use PTY string matching only before the runner has observed `SessionStart` or
  while waiting for input-ready quiescence.
- Do not auto-accept trust, login, org, billing, or setup prompts. Fail fast with
  actionable diagnostics.
- Strip ANSI/CSI control sequences before matching.
- Keep bounded excerpts in diagnostics and redact prompt text, tokens, and env
  values.
- Treat setup prompt detection as startup failure, not malformed provider
  output.
