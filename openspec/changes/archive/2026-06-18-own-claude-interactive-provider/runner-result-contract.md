## Runner Result Integration Contract

The owned Claude runner is not a print-mode stdout producer. Integration must
consume the v2 structured result and then populate existing Agent Bridge task
surfaces from that result.

## MCP Side

When the Claude provider uses the host runner:

1. Build a v2 `claude_interactive` request.
2. Send it over the configured host-runner socket.
3. Validate that the response is v2.
4. If `status` is `success`, use `result.finalText` as the provider final
   output for `agent_result`.
5. If `status` is `failure`, map `failureCategory` and bounded diagnostics into
   the provider failure result.

The MCP side must not run legacy print-mode JSON parsing for v2 Claude host
responses. Legacy helpers that expect `{"result": ...}` on stdout are not the
success contract for this provider path.

## Task Result Shape

For successful Claude v2 runs:

- `agent_result` returns the same high-level task result envelope used by other
  providers.
- The primary text body is `result.finalText`.
- Provider diagnostics may include bounded `transcript`, `stop`, and PTY
  metadata.
- Smoke checks succeed only when `result.finalText` contains
  `AGENT_BRIDGE_PROVIDER_SMOKE_OK` after Stop/transcript completion.

For failed Claude v2 runs:

- The task is marked failed.
- `failureCategory` is preserved in the provider diagnostics.
- Partial assistant text from StopFailure may be included only as redacted
  diagnostic context, never as a successful result.
- Prompt text, tokens, auth material, hook payloads, and environment values stay
  redacted.

## Host Runner Side

The host runner owns:

- launching interactive Claude;
- terminal probe handling;
- prompt injection;
- hook relay;
- transcript parsing and fallback selection;
- StopFailure classification;
- timeout and process-tree cleanup;
- producing exactly one v2 structured response.

The host runner should not emit protocol-relevant data on stdout. Logs are
diagnostic only and must remain bounded/redacted.

## Compatibility Boundary

The old host response contract for structured `claude-p` execution is legacy
v1. The new Claude provider must not silently reinterpret v1 stdout as v2
success, and the host runner must not accept v1 request shapes for owned
interactive Claude work.
