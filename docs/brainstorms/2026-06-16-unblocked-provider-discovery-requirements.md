---
date: 2026-06-16
topic: unblocked-provider-discovery
type: requirements
---

# Unblocked Provider Discovery Requirements

## Summary

Agent Bridge should let delegated ACP providers run with the same workspace filesystem reach expected by the current agent, without requiring users to register every provider binary manually. Providers that need permission-bypass flags may use an explicit unblocked launch profile, but the bridge still validates workspace scope before launch and proves provider reach with a smoke check.

---

## Problem Frame

The current bridge model protects callers with configured workspace roots and ACP-only provider launches. That avoids arbitrary command execution, but it creates two friction points: providers may be blocked by their own sandbox even inside the current workspace, and users must configure provider binaries that could be discovered safely.

The desired behavior is not a general sandbox escape. It is a predictable delegation path where eligible providers can operate inside the same workspace envelope as the current agent, and ineligible providers are hidden or reported as unavailable before real work starts.

---

## Key Decisions

- **ACP-only discovery.** Dynamic discovery applies only to known ACP-capable provider entrypoints, preserving the existing provider-launch constraint.
- **Explicit unblocked profile.** Permission-bypass flags such as `--dangerously-skip-permissions` or provider equivalents are used only when the caller chooses an unblocked profile.
- **Workspace validation remains authoritative.** The bridge validates `cwd` against the current workspace envelope before launching any provider, including unblocked launches.
- **Smoke-proven availability.** A provider is advertised as launchable only after a bounded smoke check proves it can operate in the target workspace.

---

## Requirements

**Workspace Reach**

- R1. The bridge must derive the default allowed workspace from the current agent workspace when no explicit bridge workspace configuration is provided.
- R2. The bridge must reject provider launches whose `cwd` falls outside the effective workspace envelope.
- R3. The bridge must treat provider permission failures inside the effective workspace as provider unavailability, not as a reason to relax workspace validation.

**Provider Discovery**

- R4. The bridge must discover only known ACP-capable provider commands from explicit environment settings and safe PATH defaults.
- R5. The bridge must run a bounded provider smoke check before marking a discovered provider launchable.
- R6. The bridge must not discover or launch arbitrary prompt-mode agent commands.

**Unblocked Profile**

- R7. The bridge must expose an explicit unblocked launch profile for providers that need permission-bypass flags to match the current workspace reach.
- R8. The unblocked profile must add provider-specific permission-bypass flags only after workspace validation succeeds.
- R9. The unblocked profile must be opt-in and visible in provider diagnostics, previews, and results.
- R10. The normal launch profile must remain available and must not silently inherit unblocked behavior.

---

## Acceptance Examples

- AE1. **Covers R1, R2.**
  - **Given:** Agent Bridge is running from a workspace and no explicit bridge workspace list is configured.
  - **When:** a caller spawns a provider task in that workspace.
  - **Then:** the launch is accepted using the current workspace as the effective allowed root.

- AE2. **Covers R3, R5, R7.**
  - **Given:** a provider is discoverable but cannot write a smoke-check file inside the target workspace under the requested profile.
  - **When:** provider availability is reported.
  - **Then:** the provider is marked unavailable for that profile with a permission diagnostic.

- AE3. **Covers R6, R8, R10.**
  - **Given:** a provider has both ACP and prompt-mode command shapes installed.
  - **When:** discovery runs.
  - **Then:** only the ACP shape is eligible, and unblocked flags are attached only through the explicit unblocked profile.

---

## Scope Boundaries

- No generic arbitrary-agent discovery.
- No prompt-mode fallback for providers that fail ACP discovery.
- No promise that unblocked providers have exactly the same permissions as the current agent; the bridge promises workspace validation plus smoke-proven workspace reach.
- No automatic use of unblocked mode for read-only review or research tasks.

---

## Dependencies / Assumptions

- Provider-specific ACP commands expose stable enough flags for version checks and smoke checks.
- Some providers may still impose sandbox limits that cannot be bypassed safely or portably.
- Existing denial detection remains useful for classifying providers that appear launchable but fail during real work.

---

## Sources / Research

- `crates/agent-bridge-mcp/src/task/spawn.rs` validates task `cwd` against configured workspace roots.
- `crates/agent-bridge-mcp/src/provider.rs` currently resolves ACP commands from provider-specific environment variables and defaults.
- `crates/agent-bridge-mcp/src/task/acp.rs` launches ACP children with a cleared and allowlisted environment.
- `docs/SECURITY.md` documents workspace confinement and explicit environment filtering.
