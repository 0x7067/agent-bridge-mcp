## Context

Agent Bridge has two different kinds of provider knowledge:

- Runtime knowledge: supported providers and modes, command construction, environment policy, workspace isolation, host-runner launch strategy, readiness checks, task state, logs, diffs, and result inspection.
- Runbook knowledge: how a human or caller should directly invoke a provider CLI, which flags are safe, which flags are dangerous, what output to expect, and how to troubleshoot provider-specific behavior outside the MCP lifecycle.

The runtime knowledge belongs in Agent Bridge because it is required for MCP task execution and must be tested as part of the binary. The runbook knowledge changes more often and is easier to maintain as agent skills. The current personal examples under `~/.claude/skills` prove the shape, but personal skill installs should not become this repo's source of truth.

## Goals / Non-Goals

**Goals:**

- Create a repo-owned provider skill source for Claude, Codex, Cursor, and the Kimi-backed `pi` CLI.
- Make each provider skill explicit about direct CLI invocation, install checks, safety flags, read-only vs write-capable usage, and expected evidence after execution.
- Keep Agent Bridge as the MCP runtime owner for command descriptors, lifecycle, readiness checks, isolation, and result inspection.
- Update MCP guidance so callers know when to use Agent Bridge tools versus when to consult a provider skill.
- Add validation that skill coverage and provider metadata do not silently drift.

**Non-Goals:**

- Do not replace `task_spawn`, `task_wait`, `task_logs`, `task_result`, `providers_check`, or `doctor` with skills.
- Do not make runtime command construction read arbitrary skill markdown at startup.
- Do not add a third-party plugin system for providers.
- Do not make personal `~/.claude/skills` or global Codex skills the canonical repo source.
- Do not preserve public MCP tool names, argument names, or task lifecycle response shapes solely for compatibility; any breaking cleanup must directly support the provider-skill/runtime boundary and be covered by tests/specs.

## Decisions

### Decision 1: Use repo-owned skill source files

Provider skills will live in `.codex/skills/<skill-name>/SKILL.md`. Most skill names match the runtime provider id, but the Kimi provider uses `.codex/skills/pi-agent/SKILL.md` because the direct CLI runbook is for the local `pi` command. The frontmatter `provider_id` links that skill back to runtime provider `kimi`. These files are the source of truth for provider runbooks and can be copied, symlinked, or packaged into host-specific skill locations.

Alternative considered: keep using `~/.claude/skills` directly. That is fast locally but makes the project depend on Pedro's personal machine state and prevents CI validation.

Alternative considered: use a neutral `agent-skills/` source tree. That would be host-agnostic, but this repo already has `.codex/skills` as the local skill convention and OpenSpec skills already live there.

Alternative considered: store provider runbooks only in README. That preserves repository ownership but loses the agent-skill trigger metadata and makes reuse across agents weaker.

### Decision 2: Skills own direct CLI runbooks; provider adapters own execution

Provider skills will describe how to run provider CLIs directly and safely. Provider adapters remain the only runtime source for MCP task execution commands, environment allowlists, readiness probes, launch strategies, and provider option validation.

This prevents a dangerous dependency where runtime behavior changes because prose in a skill changed. Skills can mirror runtime-supported provider names and modes, but the binary does not parse skills to decide how to spawn tasks.

### Decision 3: Expose the boundary in Agent Bridge guidance

MCP prompts/resources and README guidance will distinguish two workflows:

- Agent Bridge workflow: use `doctor`, `providers_check`, `task_preview`, `task_spawn`, `task_wait`, `task_logs`, `task_result`, and `task_remove` for MCP-native delegation.
- Direct provider workflow: consult the provider skill when an operator wants a one-shot CLI call, provider-specific flag details, or manual troubleshooting outside Agent Bridge.

The guidance should avoid duplicating full provider CLI runbooks. It should name the relevant skill and explain what category of information belongs there.

### Decision 4: Validate skill coverage and metadata drift

Tests should assert that every first-class provider has exactly one provider skill and that each skill includes required frontmatter and sections. Tests should also assert that skill-declared provider names and supported modes match provider metadata exposed by the runtime.

Each provider skill frontmatter will include these required YAML keys:

- `name`: stable skill name, such as `claude-agent` or `pi-agent`
- `description`: short trigger description
- `provider_id`: runtime provider id, one of `claude`, `codex`, `cursor`, or `kimi`
- `provider_cli`: direct CLI binary or wrapper documented by the skill, such as `claude`, `codex`, `cursor-agent`, or `pi`
- `supported_modes`: array of Agent Bridge modes supported by the runtime provider

The `pi-agent` skill is the exception to name/id symmetry: it uses `name: pi-agent`, `provider_id: kimi`, `provider_cli: pi`, and a `pinned_model` frontmatter key. Its default direct invocation must include the pinned Kimi model, for example `pi -p --model <pinned_model> "$ARGUMENT"`.

Validation should inspect repo files, not global user skill installs. It should parse the YAML frontmatter directly and should not require a separate `skills.yaml` index, because a separate index would become a second metadata source that can drift from the files agents actually load.

### Decision 5: Keep provider skill content operational and safety-biased

Each provider skill should include:

- Provider identity and trigger description.
- Install/version check command.
- Default safe direct invocation.
- Write-capable or auto-approval flags, clearly marked as dangerous and requiring explicit user authorization.
- Mode mapping to Agent Bridge concepts where useful.
- Output/evidence expectations after the subprocess exits.
- Troubleshooting notes for known provider-specific hazards.

The skills should be short enough for agents to load during work. Detailed MCP lifecycle guidance remains in Agent Bridge prompts/resources and README.

### Decision 6: Document host sync instead of adding a sync helper

The first pass will document manual copy/symlink guidance for installing repo-owned provider skills into host-specific locations such as `~/.claude/skills`. It will not add a sync helper yet.

This keeps the change focused on the source-of-truth boundary and validation. A sync helper can be added later if manual installation becomes repetitive or error-prone.

## Risks / Trade-offs

- [Risk] Skill content and runtime adapter behavior drift over time. -> Mitigation: validate provider names, modes, and required sections in CI and keep runtime command construction as the authority for execution.
- [Risk] Duplicating CLI flags in both skills and provider adapters creates maintenance work. -> Mitigation: skills should document direct invocation and safety semantics, not every internal adapter argument.
- [Risk] Agents might use direct provider skills when Agent Bridge isolation is required. -> Mitigation: guidance must state that write-capable delegated implementation should prefer Agent Bridge with managed worktree isolation.
- [Risk] Host-specific skill formats differ across Claude, Codex, Cursor, and Pi. -> Mitigation: keep the repo source in simple `SKILL.md` files with portable frontmatter and document host-specific installation as a derived concern.
- [Risk] Validation becomes brittle if skills are prose-heavy. -> Mitigation: validate small stable markers: frontmatter name/description, provider id, supported modes, safety section, install check, default invocation, dangerous flags, and evidence expectations.

## Migration Plan

1. Add failing validation tests for the expected provider skill schema and runtime-provider coverage before adding the skill files.
2. Add provider skill files for `claude-agent`, `codex-agent`, `cursor-agent`, and `pi-agent` under `.codex/skills/`. The `pi-agent` skill uses `provider_id: kimi`, `provider_cli: pi`, and a pinned Kimi model in its default direct invocation.
3. Add deterministic validation for skill coverage and required metadata/sections.
4. Update README and MCP guidance resources to point provider-specific direct CLI usage to the skills while preserving Agent Bridge lifecycle workflows.
5. Remove or deduplicate provider-specific CLI runbook details from README and MCP guidance where those details now belong in skills.
6. Keep existing personal skills untouched; optionally document how to sync repo-owned skills into personal locations.
7. Run OpenSpec validation and the default test suite.

Rollback should remove the repo-owned skill files, validation, guidance references, and any intentional runtime contract cleanup introduced by this change. Public MCP runtime behavior is not assumed to remain unchanged when cleanup materially improves the boundary.

## Open Questions

- Which exact Kimi model id should `pi-agent` pin for long-term use if the local `pi` catalog changes? The first implementation should use the currently working Kimi model id already used by local Kimi review tooling.
