use agent_bridge_mcp::provider;
use agent_bridge_mcp::{domain::TaskMode, provider::ProviderTask};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct ProviderSkill {
    path: PathBuf,
    frontmatter: BTreeMap<String, FrontmatterValue>,
    body: String,
}

#[derive(Debug, Clone)]
enum FrontmatterValue {
    Scalar(String),
    List(Vec<String>),
}

impl ProviderSkill {
    fn scalar(&self, key: &str) -> &str {
        match self.frontmatter.get(key) {
            Some(FrontmatterValue::Scalar(value)) => value,
            _ => panic!(
                "{} missing scalar frontmatter key {key}",
                self.path.display()
            ),
        }
    }

    fn optional_scalar(&self, key: &str) -> Option<&str> {
        match self.frontmatter.get(key) {
            Some(FrontmatterValue::Scalar(value)) => Some(value),
            _ => None,
        }
    }

    fn list(&self, key: &str) -> Vec<&str> {
        match self.frontmatter.get(key) {
            Some(FrontmatterValue::List(values)) => values.iter().map(String::as_str).collect(),
            _ => panic!("{} missing list frontmatter key {key}", self.path.display()),
        }
    }
}

#[test]
fn repo_provider_skills_cover_runtime_providers() {
    let skills = provider_skills();
    let mut skills_by_provider: BTreeMap<&str, Vec<&ProviderSkill>> = BTreeMap::new();
    for skill in &skills {
        skills_by_provider
            .entry(skill.scalar("provider_id"))
            .or_default()
            .push(skill);
    }

    for metadata in provider::metadata() {
        let matching = skills_by_provider
            .get(metadata.provider.as_str())
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            matching.len(),
            1,
            "expected exactly one provider skill for {}",
            metadata.provider.as_str()
        );

        let skill = matching[0];
        assert_eq!(skill.scalar("provider_cli"), metadata.provider_cli);
        assert_eq!(
            skill.list("supported_modes"),
            metadata
                .supported_modes
                .iter()
                .map(|mode| mode.as_str())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn provider_skill_frontmatter_and_sections_are_valid() {
    for skill in provider_skills() {
        assert!(!skill.scalar("name").is_empty());
        assert!(!skill.scalar("description").is_empty());
        assert!(!skill.scalar("provider_id").is_empty());
        assert!(!skill.scalar("provider_cli").is_empty());
        assert!(!skill.list("supported_modes").is_empty());

        for section in [
            "## Install Check",
            "## Safe Default Invocation",
            "## Dangerous Flags",
            "## Safety Constraints",
            "## Agent Bridge Mode Mapping",
            "## Evidence Expectations",
            "## Troubleshooting",
            "## Agent Bridge Boundary",
        ] {
            assert!(
                skill.body.contains(section),
                "{} missing section {section}",
                skill.path.display()
            );
        }
    }
}

#[test]
fn pi_agent_documents_kimi_with_pinned_model() {
    let skill = provider_skills()
        .into_iter()
        .find(|skill| skill.scalar("name") == "pi-agent")
        .expect("pi-agent provider skill should exist");
    let pinned_model = skill
        .optional_scalar("pinned_model")
        .expect("pi-agent should pin a model");

    assert_eq!(skill.scalar("provider_id"), "kimi");
    assert_eq!(skill.scalar("provider_cli"), "pi");
    assert!(!pinned_model.is_empty());
    assert!(
        skill
            .body
            .contains(&format!("pi -p --model {pinned_model}")),
        "pi-agent safe default invocation should use pinned model {pinned_model}"
    );
}

#[test]
fn provider_skills_reject_placeholders_personal_paths_and_unsafe_dangerous_flags() {
    for skill in provider_skills() {
        for forbidden in ["TBD", "TODO", "<ARGUMENT>", "<pinned_model>", "/Users/"] {
            assert!(
                !skill.body.contains(forbidden),
                "{} contains forbidden placeholder or personal path {forbidden}",
                skill.path.display()
            );
        }

        let lower = skill.body.to_ascii_lowercase();
        let mentions_dangerous_flag = [
            "--dangerously-skip-permissions",
            "--yolo",
            "--force",
            "auto-approval",
            "broad filesystem",
            "unattended write",
            "workspace-write",
            "bash",
            "edit",
            "write",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        if mentions_dangerous_flag {
            assert!(
                lower.contains("explicit user authorization"),
                "{} dangerous flag wording lacks explicit user authorization language",
                skill.path.display()
            );
        }
    }
}

#[test]
fn provider_skill_validation_rejects_malformed_or_drifting_metadata() {
    let skills = provider_skills();
    validate_skill_set(&skills).unwrap();

    assert!(
        std::panic::catch_unwind(|| parse_skill(PathBuf::from("broken.md"), "no frontmatter"))
            .is_err()
    );

    let mut missing = skills.clone();
    missing.retain(|skill| skill.scalar("provider_id") != "codex");
    assert!(validate_skill_set(&missing).unwrap_err().contains("codex"));

    let mut duplicate = skills.clone();
    duplicate.push(
        skills
            .iter()
            .find(|skill| skill.scalar("provider_id") == "codex")
            .unwrap()
            .clone(),
    );
    assert!(
        validate_skill_set(&duplicate)
            .unwrap_err()
            .contains("codex")
    );

    let mut mode_mismatch = skills.clone();
    let cursor = mode_mismatch
        .iter_mut()
        .find(|skill| skill.scalar("provider_id") == "cursor")
        .unwrap();
    cursor.frontmatter.insert(
        "supported_modes".to_string(),
        FrontmatterValue::List(vec![
            "research".to_string(),
            "review".to_string(),
            "implement".to_string(),
            "command".to_string(),
        ]),
    );
    assert!(
        validate_skill_set(&mode_mismatch)
            .unwrap_err()
            .contains("cursor")
    );

    let mut missing_section = skills.clone();
    missing_section[0].body = missing_section[0]
        .body
        .replace("## Troubleshooting", "## Debugging");
    assert!(
        validate_skill_set(&missing_section)
            .unwrap_err()
            .contains("Troubleshooting")
    );

    let mut pi_without_pin = skills.clone();
    let pi = pi_without_pin
        .iter_mut()
        .find(|skill| skill.scalar("name") == "pi-agent")
        .unwrap();
    pi.frontmatter.remove("pinned_model");
    assert!(
        validate_skill_set(&pi_without_pin)
            .unwrap_err()
            .contains("pinned_model")
    );
}

#[test]
fn provider_skill_guidance_matches_providers_list_metadata() {
    let capabilities = provider::capabilities();
    for metadata in provider::metadata() {
        let provider = metadata.provider.as_str();
        let modes = capabilities[provider]["modes"]
            .as_array()
            .unwrap_or_else(|| panic!("providers_list metadata missing modes for {provider}"));
        let capability_modes = modes
            .iter()
            .map(Value::as_str)
            .collect::<Option<Vec<_>>>()
            .unwrap();
        let runtime_modes = metadata
            .supported_modes
            .iter()
            .map(|mode| mode.as_str())
            .collect::<Vec<_>>();

        assert_eq!(runtime_modes, capability_modes);
    }
}

fn validate_skill_set(skills: &[ProviderSkill]) -> Result<(), String> {
    let mut skills_by_provider: BTreeMap<&str, Vec<&ProviderSkill>> = BTreeMap::new();
    for skill in skills {
        skills_by_provider
            .entry(skill.scalar("provider_id"))
            .or_default()
            .push(skill);
    }

    for metadata in provider::metadata() {
        let matching = skills_by_provider
            .get(metadata.provider.as_str())
            .cloned()
            .unwrap_or_default();
        if matching.len() != 1 {
            return Err(format!(
                "expected exactly one provider skill for {}",
                metadata.provider.as_str()
            ));
        }
        let skill = matching[0];
        if skill.scalar("provider_cli") != metadata.provider_cli {
            return Err(format!(
                "{} provider_cli does not match runtime metadata",
                metadata.provider.as_str()
            ));
        }
        let skill_modes = skill.list("supported_modes");
        let runtime_modes = metadata
            .supported_modes
            .iter()
            .map(|mode| mode.as_str())
            .collect::<Vec<_>>();
        if skill_modes != runtime_modes {
            return Err(format!(
                "{} supported_modes do not match runtime metadata",
                metadata.provider.as_str()
            ));
        }

        for section in [
            "## Install Check",
            "## Safe Default Invocation",
            "## Dangerous Flags",
            "## Safety Constraints",
            "## Agent Bridge Mode Mapping",
            "## Evidence Expectations",
            "## Troubleshooting",
            "## Agent Bridge Boundary",
        ] {
            if !skill.body.contains(section) {
                return Err(format!("{} missing {section}", skill.path.display()));
            }
        }
    }

    let pi = skills
        .iter()
        .find(|skill| skill.scalar("name") == "pi-agent")
        .ok_or_else(|| "pi-agent provider skill should exist".to_string())?;
    let pinned_model = pi
        .optional_scalar("pinned_model")
        .ok_or_else(|| "pi-agent missing pinned_model".to_string())?;
    if pinned_model.is_empty() {
        return Err("pi-agent pinned_model is empty".to_string());
    }

    Ok(())
}

#[test]
fn runtime_task_execution_modules_do_not_read_provider_skill_markdown() {
    for relative_path in [
        "crates/agent-bridge-mcp/src/provider.rs",
        "crates/agent-bridge-mcp/src/task.rs",
        "crates/agent-bridge-mcp/src/server.rs",
        "crates/agent-bridge-mcp/src/tools.rs",
    ] {
        let path = repo_root().join(relative_path);
        let source = fs::read_to_string(&path).unwrap();
        for forbidden in [".codex/skills", "SKILL.md", "provider_skill_guidance"] {
            assert!(
                !source.contains(forbidden),
                "{relative_path} should not read provider skill markdown or validation modules"
            );
        }
    }
}

#[test]
fn provider_commands_are_built_from_runtime_adapters_not_skill_text() {
    let codex = provider::build_command(&ProviderTask {
        provider: agent_bridge_mcp::domain::ProviderKind::Codex,
        mode: TaskMode::Review,
        prompt: "review only",
        title: None,
        cwd: "/tmp/project",
        timeout_seconds: 30,
        model: None,
        effort: None,
        thinking: None,
    })
    .unwrap();
    assert_eq!(codex.command, "codex");
    assert_eq!(codex.args[0], "exec");
    assert!(codex.args.contains(&"--sandbox".to_string()));
    assert!(codex.args.contains(&"read-only".to_string()));
    assert!(
        !codex
            .args
            .iter()
            .any(|arg| arg.contains("codex-agent") || arg.contains("SKILL.md"))
    );

    let cursor = provider::build_command(&ProviderTask {
        provider: agent_bridge_mcp::domain::ProviderKind::Cursor,
        mode: TaskMode::Review,
        prompt: "review only",
        title: None,
        cwd: "/tmp/project",
        timeout_seconds: 30,
        model: None,
        effort: None,
        thinking: None,
    })
    .unwrap();
    assert_eq!(cursor.command, "cursor-agent");
    assert!(cursor.args.contains(&"--mode".to_string()));
    assert!(cursor.args.contains(&"ask".to_string()));
    assert!(
        !cursor
            .args
            .iter()
            .any(|arg| arg.contains("cursor-agent/SKILL.md"))
    );
}

#[test]
fn readme_documents_provider_skill_boundary_and_install_source() {
    let readme = fs::read_to_string(repo_root().join("README.md")).unwrap();
    for skill_name in ["claude-agent", "codex-agent", "cursor-agent", "pi-agent"] {
        assert!(
            readme.contains(skill_name),
            "README should reference provider skill {skill_name}"
        );
    }
    assert!(readme.contains(".codex/skills/<skill-name>/SKILL.md"));
    assert!(readme.contains("derived copies"));
    assert!(readme.contains("managed worktree isolation"));
    assert!(
        readme
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .contains("provider output is evidence")
    );
    assert!(
        readme.contains("smallest\nrelevant proof") || readme.contains("smallest relevant proof")
    );
    for stale_runbook in [
        "claude -p --",
        "codex exec --",
        "cursor-agent -p --",
        "pi -p --",
    ] {
        assert!(
            !readme.contains(stale_runbook),
            "README should not embed direct provider runbook command {stale_runbook}"
        );
    }
}

#[test]
fn mcp_guidance_routes_direct_provider_cli_usage_to_skills() {
    let surfaces = [
        agent_bridge_mcp::guidance::get_prompt(serde_json::json!({
            "name": "agent_bridge_delegate_review"
        }))
        .unwrap()["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .to_string(),
        agent_bridge_mcp::guidance::get_prompt(serde_json::json!({
            "name": "agent_bridge_delegate_implementation"
        }))
        .unwrap()["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .to_string(),
        read_guidance_resource("agent-bridge://guidance/caller-workflow"),
        read_guidance_resource("agent-bridge://guidance/safety"),
        read_guidance_resource("agent-bridge://guidance/provider-capabilities"),
        read_guidance_resource("agent-bridge://guidance/dogfood-workflows"),
    ];

    let combined = surfaces.join("\n\n");
    for skill_name in ["claude-agent", "codex-agent", "cursor-agent", "pi-agent"] {
        assert!(
            combined.contains(skill_name),
            "MCP guidance should reference provider skill {skill_name}"
        );
    }
    assert!(combined.contains("direct CLI"));
    assert!(combined.contains("Agent Bridge"));
    assert!(combined.contains("managed worktree isolation"));
    assert!(combined.contains("main caller"));
    assert!(
        !combined.contains("claude -p")
            && !combined.contains("codex exec")
            && !combined.contains("cursor-agent -p")
            && !combined.contains("pi -p"),
        "MCP guidance should not embed full direct provider CLI runbooks"
    );
}

fn provider_skills() -> Vec<ProviderSkill> {
    let skills_dir = repo_root().join(".codex/skills");
    let mut skills = Vec::new();
    for entry in fs::read_dir(&skills_dir).unwrap() {
        let path = entry.unwrap().path().join("SKILL.md");
        if !path.exists() {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        if !text.starts_with("---\n") {
            continue;
        }
        let skill = parse_skill(path, &text);
        if skill.frontmatter.contains_key("provider_id") {
            skills.push(skill);
        }
    }
    skills.sort_by(|left, right| left.path.cmp(&right.path));
    skills
}

fn read_guidance_resource(uri: &str) -> String {
    agent_bridge_mcp::guidance::read_resource(serde_json::json!({ "uri": uri })).unwrap()
        ["contents"][0]["text"]
        .as_str()
        .unwrap()
        .to_string()
}

fn parse_skill(path: PathBuf, text: &str) -> ProviderSkill {
    let Some(rest) = text.strip_prefix("---\n") else {
        panic!("{} must start with YAML frontmatter", path.display());
    };
    let Some((frontmatter, body)) = rest.split_once("\n---\n") else {
        panic!("{} must close YAML frontmatter", path.display());
    };
    ProviderSkill {
        path,
        frontmatter: parse_frontmatter(frontmatter),
        body: body.to_string(),
    }
}

fn parse_frontmatter(text: &str) -> BTreeMap<String, FrontmatterValue> {
    let mut values = BTreeMap::new();
    let mut current_list_key: Option<String> = None;

    for line in text.lines() {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() || trimmed_line.starts_with('#') {
            continue;
        }
        if let Some(item) = line.trim_start().strip_prefix("- ") {
            let key = current_list_key
                .as_ref()
                .unwrap_or_else(|| panic!("list item without key: {line}"));
            match values.get_mut(key).unwrap() {
                FrontmatterValue::List(items) => items.push(unquote(item.trim())),
                FrontmatterValue::Scalar(_) => panic!("frontmatter key {key} changed type"),
            }
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            panic!("invalid frontmatter line: {line}");
        };
        let key = key.trim().to_string();
        let value = value.trim();
        if value.is_empty() {
            values.insert(key.clone(), FrontmatterValue::List(Vec::new()));
            current_list_key = Some(key);
            continue;
        }
        current_list_key = None;
        values.insert(key, FrontmatterValue::Scalar(unquote(value)));
    }

    values
}

fn unquote(value: &str) -> String {
    value
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
