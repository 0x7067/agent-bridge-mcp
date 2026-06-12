use agent_bridge_mcp::config::{Config, ConfigSources};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[test]
fn config_layers_file_env_and_cli_with_strict_precedence() {
    let root = unique_temp("config-precedence");
    let file_workspace = root.join("file-workspace");
    let env_workspace = root.join("env-workspace");
    let cli_workspace = root.join("cli-workspace");
    fs::create_dir_all(&file_workspace).unwrap();
    fs::create_dir_all(&env_workspace).unwrap();
    fs::create_dir_all(&cli_workspace).unwrap();
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        format!(
            r#"
workspaces = ["{}"]
state_dir = "{}/file-state"
claude_host_socket = "{}/file.sock"
max_active_tasks = 3
"#,
            file_workspace.display(),
            root.display(),
            root.display()
        ),
    )
    .unwrap();

    let mut env = BTreeMap::new();
    env.insert(
        "AGENT_BRIDGE_WORKSPACES".to_string(),
        OsString::from(env_workspace.as_os_str()),
    );
    env.insert(
        "AGENT_BRIDGE_STATE_DIR".to_string(),
        OsString::from(root.join("env-state").as_os_str()),
    );
    env.insert(
        "AGENT_BRIDGE_CLAUDE_HOST_SOCKET".to_string(),
        OsString::from(root.join("env.sock").as_os_str()),
    );
    env.insert(
        "AGENT_BRIDGE_MAX_ACTIVE_TASKS".to_string(),
        OsString::from("9"),
    );

    let config = Config::load(ConfigSources {
        config_path: Some(config_path),
        env,
        cli_workspaces: Some(vec![cli_workspace.clone()]),
        cli_state_dir: Some(root.join("cli-state")),
        cli_claude_host_socket: Some(root.join("cli.sock")),
        cli_max_active_tasks: Some(12),
        home_dir: Some(root.clone()),
    })
    .unwrap();

    assert_eq!(
        config.workspaces(),
        &[cli_workspace.canonicalize().unwrap()]
    );
    assert_eq!(
        config.configured_workspace_roots(),
        &[cli_workspace.canonicalize().unwrap()]
    );
    assert_eq!(config.state_dir(), root.join("cli-state"));
    assert_eq!(
        config.claude_host_socket(),
        Some(root.join("cli.sock").as_path())
    );
    assert_eq!(config.max_active_tasks(), 12);
}

#[test]
fn config_expands_home_for_state_socket_and_workspace_values() {
    let home = unique_temp("config-home");
    let workspace = home.join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let config_path = home.join("config.toml");
    fs::write(
        &config_path,
        r#"
workspaces = ["~/workspace"]
state_dir = "~/.agent-bridge-mcp/state"
claude_host_socket = "~/.agent-bridge-mcp/run/claude.sock"
"#,
    )
    .unwrap();

    let config = Config::load(ConfigSources {
        config_path: Some(config_path),
        home_dir: Some(home.clone()),
        ..ConfigSources::default()
    })
    .unwrap();

    assert_eq!(config.workspaces(), &[workspace.canonicalize().unwrap()]);
    assert_eq!(config.state_dir(), home.join(".agent-bridge-mcp/state"));
    assert_eq!(
        config.claude_host_socket(),
        Some(home.join(".agent-bridge-mcp/run/claude.sock").as_path())
    );
}

#[test]
fn config_expands_user_home_shorthand_relative_to_home_parent() {
    let root = unique_temp("config-user-home");
    let users_dir = root.join("Users");
    let home = users_dir.join("pedro");
    let other_home = users_dir.join("ana");
    let workspace = other_home.join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(&home).unwrap();
    let config_path = home.join("config.toml");
    fs::write(
        &config_path,
        r#"
workspaces = ["~ana/workspace"]
"#,
    )
    .unwrap();

    let config = Config::load(ConfigSources {
        config_path: Some(config_path),
        home_dir: Some(home),
        ..ConfigSources::default()
    })
    .unwrap();

    assert_eq!(config.workspaces(), &[workspace.canonicalize().unwrap()]);
}

#[test]
fn config_missing_file_uses_defaults_and_current_workspace() {
    let cwd = std::env::current_dir().unwrap().canonicalize().unwrap();
    let home = unique_temp("config-defaults");

    let config = Config::load(ConfigSources {
        config_path: Some(home.join("missing.toml")),
        home_dir: Some(home.clone()),
        ..ConfigSources::default()
    })
    .unwrap();

    assert_eq!(config.workspaces(), &[cwd]);
    assert_eq!(config.state_dir(), home.join(".agent-bridge-mcp/state"));
    assert_eq!(config.claude_host_socket(), None);
    assert_eq!(config.max_active_tasks(), 16);
}

fn unique_temp(prefix: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&path).unwrap();
    path
}
