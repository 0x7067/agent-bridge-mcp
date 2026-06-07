## MODIFIED Requirements

### Requirement: Launch profile behavior is observable
The system SHALL expose launch profile metadata in provider capabilities, previews, task status or result metadata, and diagnostics.

#### Scenario: Listing profile capabilities
- **WHEN** a caller invokes `providers_list`
- **THEN** each provider, including `antigravity`, includes supported launch profiles and reduced-configuration capability metadata.

#### Scenario: Antigravity bare profile caveat
- **WHEN** a caller previews or runs an Antigravity task with profile `bare`
- **THEN** the response reports compact prompt as applied and reports unsupported or best-effort reductions for ambient settings that Antigravity CLI does not expose as reliable print-mode flags.
