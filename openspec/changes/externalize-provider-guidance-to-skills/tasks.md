## 1. Skill Schema And Failing Validation

- [x] 1.1 Document `.codex/skills/<skill-name>/SKILL.md` as the repo-owned provider skill source path and personal global skill installs as derived copies only.
- [x] 1.2 Define the provider skill YAML frontmatter schema: `name`, `description`, `provider_id`, `provider_cli`, `supported_modes`, and optional `pinned_model` for provider skills that pin a model.
- [x] 1.3 Add failing validation coverage proving the expected provider skill files are missing before the new skills are added.
- [x] 1.4 Add failing validation coverage for malformed frontmatter, missing required sections, mismatched provider ids, mismatched modes, duplicate provider skill declarations, and `pi-agent` without a pinned Kimi model.

## 2. Provider Skill Source

- [x] 2.1 Add `claude-agent` provider skill guidance with `provider_id: claude`, supported modes, install check, safe default invocation, dangerous flag warnings, output evidence expectations, and troubleshooting notes.
- [x] 2.2 Add `codex-agent` provider skill guidance with `provider_id: codex`, supported modes, install check, safe default invocation, sandbox/write-mode warnings, output evidence expectations, and troubleshooting notes.
- [x] 2.3 Add `cursor-agent` provider skill guidance with `provider_id: cursor`, supported modes, install check, safe default invocation, trust/force/yolo warnings, output evidence expectations, and troubleshooting notes.
- [x] 2.4 Add `pi-agent` provider skill guidance with `provider_id: kimi`, `provider_cli: pi`, a pinned Kimi model, supported modes, install check, safe default invocation using `pi -p --model <pinned_model>`, tool/thinking warnings, output evidence expectations, and troubleshooting notes.

## 3. Validation Implementation

- [x] 3.1 Implement deterministic validation that discovers repo-owned provider skill files without reading `~/.claude/skills` or other personal global directories.
- [x] 3.2 Validate that exactly one provider skill exists for each runtime provider exposed by `providers_list`.
- [x] 3.3 Validate provider skill frontmatter includes stable skill name, description, `provider_id`, direct CLI binary/wrapper, supported modes, and the `pi-agent` pinned Kimi model.
- [x] 3.4 Validate each provider skill includes required operational sections for install checks, safe default invocation, dangerous flags, safety constraints, evidence expectations, and troubleshooting.
- [x] 3.5 Expose stable runtime provider metadata for provider ids, direct CLI names, and supported modes, then validate each provider skill's provider id and supported modes match it without requiring the skill name to equal the provider id or duplicating command construction/environment policy logic.
- [x] 3.6 Add deterministic lint coverage for placeholder strings (`TBD`, `TODO`, `<ARGUMENT>`, `<pinned_model>` outside examples), personal home-directory paths (`/Users/`, `~/.claude/skills` outside install docs), and dangerous-flag wording that mentions auto-approval/write/broad filesystem flags without explicit user authorization language.

## 4. Guidance Integration

- [x] 4.1 Update README provider sections to distinguish Agent Bridge MCP lifecycle workflows from direct provider skill runbooks.
- [x] 4.2 Remove or deduplicate provider-specific direct CLI runbook details from README and MCP guidance when those details now belong in provider skills.
- [x] 4.3 Update MCP caller workflow guidance to route direct CLI invocation and provider flag questions to provider skills.
- [x] 4.4 Update MCP provider capability guidance to name the corresponding provider skill for each provider.
- [x] 4.5 Update implementation and safety guidance to recommend Agent Bridge managed worktree isolation for write-capable delegated implementation instead of direct skill invocation.
- [x] 4.6 Preserve existing guidance that provider output is evidence and the main caller owns final verification.
- [x] 4.7 Add snapshot or content-scan tests proving README and MCP guidance reference the expected provider skills and no longer embed stale full direct-provider CLI runbooks.
- [x] 4.8 Document manual copy/symlink guidance for installing repo-owned provider skills into host-specific skill locations; do not implement a sync helper in this change.

## 5. Runtime Boundary Checks

- [x] 5.1 Add or update tests proving `task_preview` command descriptors are built from provider adapter runtime logic, not provider skill markdown, including a temporary skill prose mutation or static boundary check.
- [x] 5.2 Add or update tests proving `task_spawn` behavior is derived from provider adapter runtime logic, not provider skill prose, including a temporary skill prose mutation or static boundary check; do not require old behavior solely for compatibility.
- [x] 5.3 Keep provider adapter metadata as the source for provider names, modes, and provider-specific options used by validation.

## 6. Verification

- [x] 6.1 Run `openspec validate externalize-provider-guidance-to-skills`.
- [x] 6.2 Run the default Rust test suite.
- [x] 6.3 Re-read the created provider skills and guidance references for placeholders, stale personal paths, and unsafe flag wording.
