## Context

The current Claude provider defaults to upstream `claude-p`, can optionally select native `claude -p`, and can route the `claude-p` command through the existing Claude host runner. That improved sandbox/Keychain behavior, but the runtime still depends on external print-mode compatibility surfaces.

The desired provider is narrower and more owned: Agent Bridge should run the official interactive `claude` CLI in a real PTY, drive it with bridge-owned automation, capture completion through Claude Code transcript/Stop-hook behavior, and surface the result through the existing Agent Bridge task lifecycle. Upstream `claude-p` is reference material for the port, not a runtime dependency.

Docs checked before implementation are summarized in `docs-research.md`: Claude Code CLI reference, programmatic/headless usage and billing note, hooks reference, settings reference, upstream `claude-p` README/SPEC/REPORT, and the Lanes PTY architecture note. The relevant constraints are that `claude -p` is explicitly the programmatic/Agent SDK path, interactive `claude` starts a terminal session, hook payloads include transcript paths, and inline `--settings` can supply temporary hook settings without durable config edits.

## Goals / Non-Goals

**Goals:**
- Keep `claude` as the only public Claude provider name.
- Make the Claude provider use a bridge-owned interactive runner for task execution and smoke checks.
- Remove native `claude -p` and upstream `claude-p` as normal fallback paths.
- Port the required `claude-p` mechanics into Rust-owned code: PTY launch, terminal probe responses, prompt injection, Stop hook registration, transcript extraction, output rendering, timeout handling, and cleanup.
- Preserve structured host-runner safety: local Unix socket, workspace policy id, validated cwd, structured requests, bounded output, redacted diagnostics, and no arbitrary command execution.
- Keep deterministic tests independent of live Claude auth, network, Keychain, or paid model usage.

**Non-Goals:**
- Do not vendor Claude Code or patch Anthropic's CLI.
- Do not add native `claude -p` as a fallback.
- Do not keep upstream `claude-p` as a required runtime dependency.
- Do not build a general terminal automation service for arbitrary commands.
- Do not implement per-token stream parity in v1; final-result parity and useful task diagnostics are enough.

## Decisions

### Decision 1: Own the Claude provider runtime in Rust

The Claude provider will use a Rust-owned interactive runner, modeled after `claude-p` but integrated into Agent Bridge's provider and host-runner contracts. The runner invokes the official `claude` binary interactively in a PTY, never `claude -p` and never upstream `claude-p` for normal task execution.

Alternative considered: keep upstream `claude-p` and harden diagnostics. That leaves the failure-prone compatibility layer outside the bridge and prevents us from fixing the exact integration that keeps failing.

Alternative considered: native `claude -p`. Pedro explicitly rejected this as a fallback, and it is the wrong billing/reliability surface for this provider.

### Decision 2: Make host-runner execution the normal Claude path

The owned interactive runner should live behind the existing Claude host-runner boundary. The MCP process sends a structured Claude task request to the runner; the runner validates workspace policy and runs interactive Claude outside the sandbox. If a host runner is configured but unavailable, Claude fails with a host-runner diagnostic and does not fall back to print mode.

Production Claude provider execution requires the owned host-runner path. The implementation may keep a direct owned-runner harness for deterministic tests and local development, but normal MCP provider readiness must not mark Claude launchable without the host runner unless a future OpenSpec change explicitly promotes direct owned execution to production support.

The host-runner protocol for this change is version `2`. Version `2` requests use the existing structured request boundary but replace `RunClaude`/`claude-p` semantics with owned interactive Claude runner semantics. Version `1` requests or responses must fail with `protocol_mismatch` rather than being interpreted as owned-runner work. Version `2` run results include: exit status or signal, duration, failure category, bounded PTY output excerpts with truncation flags, Stop payload metadata, StopFailure metadata when present, and transcript parse diagnostics. The MCP binary and host runner should be upgraded together.

The concrete v2 schema is specified in `protocol-v2.md`. The integration
boundary between structured v2 results and existing task surfaces is specified
in `runner-result-contract.md`. Implementation must consume v2 structured
results directly; legacy print-mode stdout JSON parsing is not the success path
for owned Claude execution.

### Decision 3: Port the minimal `claude-p` mechanics, not its package boundary

The port must include the mechanics Agent Bridge needs:
- start `claude` under a PTY using the user's login shell environment,
- answer known Claude Code terminal startup probes that otherwise block the TUI,
- inject the rendered prompt without putting prompt text in argv,
- install runner-owned temporary `--settings` JSON that registers `SessionStart` and `Stop` hooks while avoiding durable edits to user/project settings,
- read the transcript path and final-message fallback from the Stop hook payload,
- classify StopFailure hook payloads as provider/API failures instead of malformed transcript output,
- extract final assistant/result information into the existing provider output parser,
- clean up temporary settings and child processes on success, timeout, disconnect, and shutdown.

The hook relay should be bridge-owned IPC, not hook stdout. V1 should use a runner-owned FIFO in a `0700` temporary directory, with hook helper commands writing newline-delimited JSON payloads only to that FIFO. Hook stdout must stay empty unless the runner intentionally wants to inject text into Claude context.

The hook relay details are specified in `hook-relay-contract.md`. The runner
uses Agent Bridge environment names, registers `SessionStart`, `Stop`, and
`StopFailure`, and opens the relay before spawning Claude so hook helpers cannot
block indefinitely waiting for a reader.

Startup sequencing is specified in `startup-sequencing.md`: start readers
before prompt injection, respond to terminal probes, wait for `SessionStart`
plus PTY quiescence, write the prompt to PTY input, then send Enter as a
separate bounded write.

The login-shell compatibility decision is intentionally narrow. The host runner
may use a fixed Agent Bridge-owned `/bin/zsh -flc 'exec "$@"'` bootstrap to
preserve the user's local Claude/PATH/Keychain environment, but the host-runner
protocol must never accept caller-supplied shell source, command strings,
arbitrary argv, or executable paths.

The port should not preserve `claude-p`'s public CLI compatibility layer unless Agent Bridge directly needs a behavior for provider execution.

### Decision 4: Keep output parity scoped to Agent Bridge needs

Agent Bridge needs a reliable final report and diagnostics, not a public drop-in `claude -p` replacement. Text/json/stream-json byte-for-byte parity is not a v1 requirement. The runner should normalize transcript output into the existing task result/log surfaces and mark unsupported parity areas explicitly in diagnostics or docs.

The minimum parser contract is: prefer the final assistant message extracted from transcript JSONL records, retry transcript reads within a bounded flush budget, fall back to Stop `last_assistant_message` if transcript parsing does not produce a final assistant message, and classify StopFailure as provider failure rather than success.

Transcript paths from hooks must be absolute, canonicalizable, regular readable files, and must not traverse through symlinks after canonicalization. V1 may accept Claude-owned transcript paths outside the workspace because Claude Code stores transcripts under its own config/state directories, but unsafe paths are rejected and the runner falls back to `last_assistant_message` when present.

### Decision 5: Evaluate a PTY dependency before implementation

The implementation should evaluate established Rust PTY crates against these criteria: macOS/Linux support, Tokio compatibility or safe thread integration, child process group cleanup, PTY resize/control-byte handling, and low maintenance risk. Add the smallest dependency that satisfies those constraints. If no crate is acceptable, isolate the unsafe/platform PTY code behind a small adapter module.

### Decision 6: Provider readiness proves the owned runner path

`providers_check(smoke: true)` for Claude must exercise the owned interactive runner path. Version-only checks may verify that the official `claude` binary is discoverable, but they must not mark Claude launchable. If the owned runner cannot start, inject the prompt, observe Stop-hook completion, or parse a transcript result, Claude readiness is failed with bounded diagnostics.

Binary resolution for the official interactive Claude executable is implementation-defined but must be explicit before runtime integration. The preferred policy is `CLAUDE_BIN` as the interactive `claude` executable override, then `PATH` lookup for `claude`. The implementation should not introduce `npx claude` fallback unless source verification shows it is stable and non-surprising.

### Decision 7: Keep Claude mode and permission mapping explicit

The adapter must map Agent Bridge modes onto interactive Claude flags supported outside print mode. Research/review should start with read-only-style permissions (`dontAsk` plus restricted tools or equivalent settings), command should allow bounded shell usage without edits, and implement should use the existing default/approval behavior unless a safer interactive mode is verified.

The owned-runner smoke prompt is the existing non-mutating token prompt: `Reply with exactly: AGENT_BRIDGE_PROVIDER_SMOKE_OK`. Smoke succeeds only when the owned runner reaches Stop/transcript completion and the parsed final assistant result contains `AGENT_BRIDGE_PROVIDER_SMOKE_OK`.

### Decision 8: Update docs and previews to remove print-mode fallback guidance

README, provider capability metadata, task previews, and troubleshooting guidance should describe the owned interactive runner. Existing references that tell users to configure native print-mode fallback or upstream wrapper fallback should be removed.

## Risks / Trade-offs

- Claude Code terminal behavior can change -> keep PTY/probe handling isolated, add fake-probe tests, and include a live smoke checklist for local validation.
- Stop hook or transcript schema can change -> classify missing/unknown payloads distinctly and keep transcript parsing covered by fixtures.
- StopFailure can be mistaken for malformed output -> parse it separately and map known error values to actionable provider failure categories.
- Host runner is a sandbox escape -> keep structured requests, workspace validation, owner-only socket permissions, bounded output, and sanitized logs.
- New PTY dependency may be brittle -> evaluate before adding, pin behavior through adapter tests, and keep the dependency behind one module.
- Interactive runner may be slower than print mode -> accept startup overhead because reliability and provider policy matter more than print-mode speed.
- Perfect `claude -p` output parity is expensive -> scope v1 to Agent Bridge task lifecycle output and explicitly document non-parity.
- Host-runner protocol drift can break all Claude tasks -> bump the protocol for owned-runner semantics, reject mismatches clearly, and document that MCP binary and host runner should be upgraded together.

## Migration Plan

1. Add deterministic fake interactive Claude fixtures that emulate terminal probes, prompt entry, Stop-hook payloads, transcript files, timeout, and malformed transcript cases.
2. Introduce an owned Claude runner module and host-runner request kind while keeping public `claude` provider APIs stable.
3. Change Claude provider command selection/readiness to use the owned interactive runner and remove upstream `claude-p`/native print-mode fallback code.
4. Update docs, previews, diagnostics, and provider metadata to report the owned runner launch strategy.
5. Run Rust tests and OpenSpec validation.
6. Build the release binary, replace the installed binary, and verify the installed MCP tool surface.
7. Start the host runner outside the sandbox and run an optional live Claude smoke check.

Rollback before release is to revert this change. After release, rollback requires restoring the previous `claude-p` host-runner path from git history; no runtime fallback should remain hidden in the new provider.

## Open Questions

- Does the `pty-process` spike pass on Pedro's macOS arm64 environment with
  split IO, probe handling, resize, and process-group cleanup before production
  runner wiring?
