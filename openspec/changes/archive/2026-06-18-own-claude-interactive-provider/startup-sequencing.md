## Claude Interactive Startup Sequencing

The owned runner follows a deterministic startup sequence so interactive Claude
is driven through a real PTY without leaking prompt text through argv,
diagnostics, or hook stdout.

## Sequence

1. Validate the v2 request, workspace policy, cwd, timeout, mode, model, effort,
   and prompt size.
2. Build runner-owned temporary settings and hook relay artifacts.
3. Resolve the official interactive `claude` binary from `CLAUDE_BIN` or
   `PATH`.
4. Spawn Claude in a PTY at a fixed initial size, default `120x40`.
5. Start PTY reader and hook relay reader before prompt injection.
6. Respond to known terminal probes while keeping probe bytes out of MCP stdout.
7. Detect setup/login/trust prompts before normal timeout.
8. Wait for `SessionStart` and PTY output quiescence.
9. Write the rendered prompt bytes to PTY input.
10. After a short bounded delay, write Enter (`\r`) as a separate PTY write.
11. Wait for `Stop` or `StopFailure`, child exit, timeout, or disconnect.
12. Parse transcript/fallback data and return one v2 structured result.
13. Clean up hook artifacts and the PTY child process tree.

## Terminal Probe Responses

The first implementation should cover the probes recorded from upstream
`claude-p` source:

| Probe | Response |
| --- | --- |
| `ESC[c` or `ESC[0c` | `ESC[?1;2c` |
| `ESC[>c` or `ESC[>0c` | `ESC[>0;0;0c` |
| `ESC[6n` | `ESC[1;1R` |
| `ESC[>q` | `ESC P>|agent-bridge-claude ESC\\` |
| `ESC[18t` | `ESC[8;40;120t` |

Tests should assert probe bytes never appear as provider stdout.

## Input Ready

Input readiness is a conjunction:

- hook relay observed `SessionStart`;
- PTY output reached a bounded quiescent window;
- no setup/login/trust signature has been detected;
- startup deadline has not expired.

The initial quiescent window should be short enough to avoid hiding startup
failures and long enough to avoid injecting the prompt into active terminal
paint. The implementation should start with 80-150 ms quiescence and keep the
overall startup budget explicit.

## Login Shell Bootstrap

The host runner may use a constant, runner-owned login shell bootstrap to
preserve the user's `PATH`, Keychain, and Claude auth environment:

```text
/bin/zsh -flc 'exec "$@"' agent-bridge-claude <claude-bin> <args...>
```

This shell source is fixed by Agent Bridge. User-controlled data is passed only
as positional argv, cwd, env, or PTY input. The host-runner protocol never
accepts caller-supplied shell source, command strings, arbitrary argv, or
executable paths.

If the PTY adapter can directly spawn the resolved `claude` binary with the
required login-compatible environment in tests, direct structured spawn remains
preferred. The implementation must document which path is used.

## Failure Classification

- Setup/login/trust prompt before input-ready: `claude_setup_required`.
- Startup deadline before input-ready: `startup_timeout`.
- Hook relay setup/read failure: `hook_relay_error`.
- StopFailure: mapped Claude provider/API category.
- Timeout after prompt injection: `runner_timeout`.
- No usable Stop/transcript/fallback after child exit: `provider_output_error`
  or `transcript_error`, depending on the failing component.
