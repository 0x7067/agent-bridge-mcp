// Diagnostics surface: `doctor`, provider smoke/readiness checks, binary
// freshness, and client-config inspection. Extracted from server.rs;
// consolidates the doctor and providers helper families.
// `use super::*` gives access to server.rs's private helpers (child modules
// can see ancestor-private items).
use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProvidersCheckInput {
    #[serde(default)]
    smoke: bool,
    timeout_ms: Option<i64>,
    providers: Option<Vec<ProviderKind>>,
    aggregate_timeout_ms: Option<i64>,
    provider_timeout_ms: Option<BTreeMap<String, i64>>,
    cwd: Option<String>,
    profile: Option<LaunchProfile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DoctorInput {
    focus: Option<String>,
    #[serde(default)]
    smoke: bool,
    timeout_ms: Option<i64>,
    providers: Option<Vec<ProviderKind>>,
    aggregate_timeout_ms: Option<i64>,
    provider_timeout_ms: Option<BTreeMap<String, i64>>,
    cwd: Option<String>,
    profile: Option<LaunchProfile>,
}

pub(super) async fn doctor(arguments: Value) -> Result<Value, String> {
    let input: DoctorInput =
        serde_json::from_value(arguments).map_err(|error| error.to_string())?;
    match input.focus.as_deref() {
        None | Some("all") => {}
        Some("providers") => return providers_check(doctor_provider_arguments(&input)).await,
        Some(other) => {
            return Err(format!(
                "focus must be one of: all, providers (got {other})"
            ));
        }
    }
    let workspace = doctor_workspace(input.cwd.as_deref());
    let state = doctor_state();
    let orphans = doctor_orphans();
    let binary = doctor_binary(input.cwd.as_deref());
    let clients = doctor_clients();
    let task_extension_readiness = doctor_task_extension_readiness();
    let claude_host_runner = doctor_claude_host_runner().await;
    let provider_report = providers_check(doctor_provider_arguments(&input)).await?;
    let providers = provider_report
        .get("providers")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let provider_status = providers_status(&providers);
    let launch_readiness = doctor_launch_readiness(&providers, input.providers.as_deref());
    let recommendations = doctor_recommendations(
        workspace["status"].as_str().unwrap_or("ok"),
        state["status"].as_str().unwrap_or("ok"),
        provider_status,
        claude_host_runner["status"].as_str().unwrap_or("ok"),
        &launch_readiness,
        &clients,
        &binary,
        &orphans,
    );
    let summary_status = aggregate_status([
        workspace["status"].as_str().unwrap_or("ok"),
        state["status"].as_str().unwrap_or("ok"),
        provider_status,
        claude_host_runner["status"].as_str().unwrap_or("ok"),
    ]);

    Ok(json!({
        "summary": {
            "status": summary_status,
            "checkedAt": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        },
        "server": {
            "name": "agent-bridge-mcp",
            "version": "0.1.0",
            "protocolVersion": PROTOCOL_VERSION,
            "environment": doctor_environment()
        },
        "workspace": workspace,
        "state": state,
        "orphans": orphans,
        "binary": binary,
        "clients": clients,
        "taskExtensionReadiness": task_extension_readiness,
        "providers": providers,
        "launchReadiness": launch_readiness,
        "claudeHostRunner": claude_host_runner,
        "recommendations": recommendations
    }))
}

fn doctor_task_extension_readiness() -> Value {
    json!({
        "classification": "unavailable",
        "serverAdvertisesTasks": false,
        "source": "none",
        "observedExtensionIdentifiers": [],
        "legacyIndicators": [],
        "unknownIndicators": [],
        "recommendedNextStep": "Use the ACP router by default or agent_delegate through the minimal MCP adapter.",
        "checkedAt": checked_at_iso()
    })
}

fn doctor_provider_arguments(input: &DoctorInput) -> Value {
    let mut arguments = serde_json::Map::new();
    arguments.insert("smoke".to_string(), json!(input.smoke));
    if let Some(timeout_ms) = input.timeout_ms {
        arguments.insert("timeoutMs".to_string(), json!(timeout_ms));
    }
    if let Some(providers) = input.providers.as_ref() {
        arguments.insert("providers".to_string(), json!(providers));
    }
    if let Some(aggregate_timeout_ms) = input.aggregate_timeout_ms {
        arguments.insert(
            "aggregateTimeoutMs".to_string(),
            json!(aggregate_timeout_ms),
        );
    }
    if let Some(provider_timeout_ms) = input.provider_timeout_ms.as_ref() {
        arguments.insert("providerTimeoutMs".to_string(), json!(provider_timeout_ms));
    }
    if let Some(cwd) = input.cwd.as_ref() {
        arguments.insert("cwd".to_string(), json!(cwd));
    }
    if let Some(profile) = input.profile {
        arguments.insert("profile".to_string(), json!(profile));
    }
    Value::Object(arguments)
}

fn doctor_workspace(cwd: Option<&str>) -> Value {
    let roots = match doctor_configured_workspace_roots() {
        Ok(roots) => roots,
        Err(error) => {
            return json!({
                "status": "error",
                "error": error
            });
        }
    };
    let cwd_report = match safe_cwd(cwd) {
        Ok(path) => json!({
            "status": "ok",
            "path": path,
            "insideConfiguredWorkspace": true
        }),
        Err(error) => json!({
            "status": "error",
            "error": error,
            "insideConfiguredWorkspace": false
        }),
    };
    let status = if cwd_report["status"] == "error" {
        "error"
    } else {
        "ok"
    };
    json!({
        "status": status,
        "roots": roots
            .iter()
            .map(|root| root.display().to_string())
            .collect::<Vec<_>>(),
        "cwd": cwd_report
    })
}

fn doctor_configured_workspace_roots() -> Result<Vec<PathBuf>, String> {
    crate::config::Config::from_env(crate::config::ConfigCliOverrides::default())
        .map(|config| config.configured_workspace_roots().to_vec())
}

fn doctor_environment() -> Value {
    const KEYS: &[&str] = &[
        "AGENT_BRIDGE_WORKSPACES",
        "AGENT_BRIDGE_STATE_DIR",
        "AGENT_BRIDGE_CLAUDE_HOST_SOCKET",
        "AGENT_BRIDGE_INSTALLED_BIN",
        "AGENT_BRIDGE_RELEASE_BIN",
        "CODEX_BIN",
        "FORGE_BIN",
        "CURSOR_AGENT_BIN",
        "PI_BIN",
        "AGY_BIN",
        "CLAUDE_BIN",
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "ANTHROPIC_BASE_URL",
    ];
    let mut environment = serde_json::Map::new();
    for key in KEYS {
        let present = env::var_os(key).is_some_and(|value| !value.is_empty());
        let mut entry = json!({ "present": present });
        if present && is_sensitive_env_key(key) {
            entry["value"] = json!("<redacted>");
        }
        environment.insert((*key).to_string(), entry);
    }
    Value::Object(environment)
}

fn is_sensitive_env_key(key: &str) -> bool {
    let key = key.to_ascii_uppercase();
    ["TOKEN", "API_KEY", "OAUTH", "AUTH", "PASSWORD", "SECRET"]
        .iter()
        .any(|needle| key.contains(needle))
}

#[derive(Debug, Clone)]
pub(super) struct BinaryTarget {
    path: PathBuf,
    exists: bool,
    readable: bool,
    size_bytes: Option<u64>,
    modified_at: Option<String>,
    fingerprint: Option<String>,
    fingerprint_status: &'static str,
    error: Option<String>,
}

impl BinaryTarget {
    pub(super) fn inspect(path: PathBuf) -> Self {
        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() => {
                let size_bytes = metadata.len();
                let modified_at = metadata.modified().ok().map(|time| {
                    chrono::DateTime::<chrono::Utc>::from(time)
                        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                });
                let (fingerprint, fingerprint_status, error) = fingerprint_file(&path, size_bytes);
                Self {
                    path,
                    exists: true,
                    readable: fingerprint_status != "error",
                    size_bytes: Some(size_bytes),
                    modified_at,
                    fingerprint,
                    fingerprint_status,
                    error,
                }
            }
            Ok(_) => Self {
                path,
                exists: true,
                readable: false,
                size_bytes: None,
                modified_at: None,
                fingerprint: None,
                fingerprint_status: "error",
                error: Some("path is not a regular file".to_string()),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self {
                path,
                exists: false,
                readable: false,
                size_bytes: None,
                modified_at: None,
                fingerprint: None,
                fingerprint_status: "missing",
                error: None,
            },
            Err(error) => Self {
                path,
                exists: true,
                readable: false,
                size_bytes: None,
                modified_at: None,
                fingerprint: None,
                fingerprint_status: "error",
                error: Some(error.to_string()),
            },
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "path": self.path.display().to_string(),
            "exists": self.exists,
            "readable": self.readable,
            "sizeBytes": self.size_bytes,
            "modifiedAt": self.modified_at,
            "fingerprint": self.fingerprint,
            "fingerprintStatus": self.fingerprint_status,
            "error": self.error
        })
    }
}

fn fingerprint_file(
    path: &Path,
    size_bytes: u64,
) -> (Option<String>, &'static str, Option<String>) {
    if size_bytes > MAX_BINARY_FINGERPRINT_BYTES {
        return (None, "skipped_too_large", None);
    }
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => return (None, "error", Some(error.to_string())),
    };
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    (Some(format!("fnv64:{hash:016x}")), "ok", None)
}

fn doctor_binary(cwd: Option<&str>) -> Value {
    let running = match std::env::current_exe() {
        Ok(path) => BinaryTarget::inspect(path),
        Err(error) => BinaryTarget {
            path: PathBuf::new(),
            exists: false,
            readable: false,
            size_bytes: None,
            modified_at: None,
            fingerprint: None,
            fingerprint_status: "error",
            error: Some(error.to_string()),
        },
    };
    let installed = BinaryTarget::inspect(installed_binary_path());
    let release = BinaryTarget::inspect(release_binary_path(cwd));
    binary_report(running, installed, release)
}

fn installed_binary_path() -> PathBuf {
    env::var("AGENT_BRIDGE_INSTALLED_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|_| crate::config::expand_home("~/.local/bin/agent-bridge-mcp"))
}

pub(super) fn release_binary_path(cwd: Option<&str>) -> PathBuf {
    if let Ok(path) = env::var("AGENT_BRIDGE_RELEASE_BIN") {
        return PathBuf::from(path);
    }
    cwd.map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
        .join("target/release/agent-bridge-mcp")
}

pub(super) fn binary_report(
    running: BinaryTarget,
    installed: BinaryTarget,
    release: BinaryTarget,
) -> Value {
    let matches_release = binary_targets_match(&installed, &release);
    let installed_matches_running = binary_targets_match(&installed, &running);
    let release_matches_running = binary_targets_match(&release, &running);
    let status = binary_status(
        &running,
        &installed,
        &release,
        matches_release,
        installed_matches_running,
    );
    let recommendations =
        binary_recommendation_strings(status, matches_release, &installed, &release);
    let mut installed_json = installed.to_json();
    installed_json["matchesRelease"] = json!(matches_release);
    installed_json["matchesRunning"] = json!(installed_matches_running);
    let mut release_json = release.to_json();
    release_json["matchesRunning"] = json!(release_matches_running);
    json!({
        "status": status,
        "fingerprintLimitBytes": MAX_BINARY_FINGERPRINT_BYTES,
        "running": running.to_json(),
        "installed": installed_json,
        "release": release_json,
        "recommendations": recommendations
    })
}

fn binary_targets_match(left: &BinaryTarget, right: &BinaryTarget) -> bool {
    left.readable
        && right.readable
        && left.size_bytes == right.size_bytes
        && left.fingerprint_status == "ok"
        && right.fingerprint_status == "ok"
        && left.fingerprint == right.fingerprint
}

fn binary_status(
    running: &BinaryTarget,
    installed: &BinaryTarget,
    release: &BinaryTarget,
    matches_release: bool,
    installed_matches_running: bool,
) -> &'static str {
    if running.fingerprint_status == "error"
        || override_path_error("AGENT_BRIDGE_INSTALLED_BIN", installed)
        || override_path_error("AGENT_BRIDGE_RELEASE_BIN", release)
    {
        return "error";
    }
    if !installed.exists
        || !release.exists
        || !matches_release
        || !installed_matches_running
        || [running, installed, release]
            .iter()
            .any(|target| target.fingerprint_status == "skipped_too_large")
    {
        return "warning";
    }
    if installed.readable && release.readable && matches_release {
        "ok"
    } else {
        "unknown"
    }
}

fn override_path_error(key: &str, target: &BinaryTarget) -> bool {
    env::var_os(key).is_some()
        && target.exists
        && (!target.readable || target.fingerprint_status == "error")
}

fn binary_recommendation_strings(
    status: &str,
    matches_release: bool,
    installed: &BinaryTarget,
    release: &BinaryTarget,
) -> Vec<String> {
    let mut recommendations = Vec::new();
    if !release.exists {
        recommendations.push(
            "Build the release binary with cargo build --release --bin agent-bridge-mcp before comparing freshness."
                .to_string(),
        );
    }
    if !installed.exists {
        recommendations.push(
            "Install the release binary to the configured installed binary path.".to_string(),
        );
    }
    if status == "warning" && installed.exists && release.exists && !matches_release {
        recommendations.push(
            "Rebuild and install the release binary so the installed copy matches target/release."
                .to_string(),
        );
    }
    recommendations
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClientKind {
    Codex,
    Claude,
    Cursor,
}

impl ClientKind {
    fn name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Cursor => "cursor",
        }
    }

    fn config_path(self, home: &Path) -> PathBuf {
        match self {
            Self::Codex => home.join(".codex/config.toml"),
            Self::Claude => home.join(".claude.json"),
            Self::Cursor => home.join(".cursor/mcp.json"),
        }
    }

    fn verification_command(self) -> Option<Vec<&'static str>> {
        match self {
            Self::Codex => Some(vec!["codex", "mcp", "list"]),
            Self::Claude => Some(vec!["claude", "mcp", "list"]),
            Self::Cursor => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct ClientRegistration {
    pub(super) command: Option<String>,
    pub(super) args: Vec<String>,
    pub(super) env_keys: Vec<String>,
    pub(super) similar_registrations: Vec<String>,
}

struct ClientReport {
    client: ClientKind,
    config_path: String,
    config_present: bool,
    parse_status: &'static str,
    registration_status: &'static str,
    command: Option<Value>,
    args: Vec<String>,
    env_keys: Vec<String>,
    similar_registrations: Vec<String>,
    status: &'static str,
    recommendations: Vec<String>,
    error: Option<String>,
}

fn doctor_clients() -> Value {
    let home = env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("~"));
    doctor_clients_from_home(&home)
}

pub(super) fn doctor_clients_from_home(home: &Path) -> Value {
    let mut clients = serde_json::Map::new();
    for client in [ClientKind::Codex, ClientKind::Claude, ClientKind::Cursor] {
        clients.insert(client.name().to_string(), doctor_client(client, home));
    }
    Value::Object(clients)
}

fn doctor_client(client: ClientKind, home: &Path) -> Value {
    let path = client.config_path(home);
    let path_text = path.display().to_string();
    let contents = match read_client_config(&path) {
        Ok(Some(contents)) => contents,
        Ok(None) => {
            return client_report(ClientReport {
                client,
                config_path: path_text.clone(),
                config_present: false,
                parse_status: "missing",
                registration_status: "absent",
                command: None,
                args: Vec::new(),
                env_keys: Vec::new(),
                similar_registrations: Vec::new(),
                status: "info",
                recommendations: vec![format!(
                    "No {} user-level MCP config file was found; add Agent Bridge there only if you use this client.",
                    client.name()
                )],
                error: None,
            });
        }
        Err(error) => {
            return client_report(ClientReport {
                client,
                config_path: path_text.clone(),
                config_present: true,
                parse_status: "error",
                registration_status: "absent",
                command: None,
                args: Vec::new(),
                env_keys: Vec::new(),
                similar_registrations: Vec::new(),
                status: "error",
                recommendations: vec![format!(
                    "Inspect {} because it could not be read: {error}.",
                    path_text,
                )],
                error: Some(error),
            });
        }
    };

    let parsed = match client {
        ClientKind::Codex => parse_codex_registration(&contents),
        ClientKind::Claude | ClientKind::Cursor => parse_json_registration(&contents),
    };
    let registration = match parsed {
        Ok(Some(registration)) => registration,
        Ok(None) => {
            return client_report(ClientReport {
                client,
                config_path: path_text.clone(),
                config_present: true,
                parse_status: "ok",
                registration_status: "absent",
                command: None,
                args: Vec::new(),
                env_keys: Vec::new(),
                similar_registrations: Vec::new(),
                status: "info",
                recommendations: vec![format!(
                    "No canonical agent-bridge MCP registration was found in {}.",
                    path_text,
                )],
                error: None,
            });
        }
        Err(error) => {
            return client_report(ClientReport {
                client,
                config_path: path_text,
                config_present: true,
                parse_status: "error",
                registration_status: "absent",
                command: None,
                args: Vec::new(),
                env_keys: Vec::new(),
                similar_registrations: Vec::new(),
                status: "error",
                recommendations: vec![format!("Fix the {} config parse error.", client.name())],
                error: Some(error),
            });
        }
    };

    let command = command_diagnostic(registration.command.as_deref());
    let status = if command["status"].as_str() == Some("warning") {
        "warning"
    } else {
        "ok"
    };
    let mut recommendations = Vec::new();
    if status == "warning" {
        recommendations.push(format!(
            "Inspect the {} Agent Bridge command configuration.",
            client.name()
        ));
    }
    let verification_commands = verification_commands(client);
    if !verification_commands.is_empty() {
        recommendations.push(format!(
            "Run {} to verify the {} client can load the registered MCP server.",
            verification_commands[0]["command"]
                .as_array()
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default(),
            client.name()
        ));
    }
    client_report(ClientReport {
        client,
        config_path: path_text,
        config_present: true,
        parse_status: "ok",
        registration_status: "registered",
        command: Some(command),
        args: registration.args,
        env_keys: registration.env_keys,
        similar_registrations: registration.similar_registrations,
        status,
        recommendations,
        error: None,
    })
}

fn read_client_config(path: &Path) -> Result<Option<String>, String> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if metadata.len() > MAX_CLIENT_CONFIG_BYTES {
        return Err(format!(
            "config file exceeds {} bytes",
            MAX_CLIENT_CONFIG_BYTES
        ));
    }
    std::fs::read_to_string(path)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn client_report(mut report: ClientReport) -> Value {
    report.env_keys.sort();
    report.env_keys.dedup();
    let verification_commands = if report.registration_status == "registered" {
        verification_commands(report.client)
    } else {
        Vec::new()
    };
    let mut value = json!({
        "client": report.client.name(),
        "status": report.status,
        "configPath": report.config_path,
        "configPresent": report.config_present,
        "parseStatus": report.parse_status,
        "registrationStatus": report.registration_status,
        "command": report.command.unwrap_or_else(|| command_diagnostic(None)),
        "args": report.args,
        "envKeys": report.env_keys,
        "verificationStatus": "not_verified",
        "verificationCommands": verification_commands,
        "recommendations": report.recommendations
    });
    if !report.similar_registrations.is_empty() {
        value["similarRegistrations"] = json!(report.similar_registrations);
    }
    if let Some(error) = report.error {
        value["error"] = json!(error);
    }
    value
}

fn verification_commands(client: ClientKind) -> Vec<Value> {
    client
        .verification_command()
        .into_iter()
        .map(|command| {
            json!({
                "kind": "shell",
                "command": command,
                "description": format!(
                    "Verify {} can load the registered Agent Bridge MCP server.",
                    client.name()
                )
            })
        })
        .collect()
}

pub(super) fn command_diagnostic(command: Option<&str>) -> Value {
    let Some(command) = command.filter(|command| !command.is_empty()) else {
        return json!({
            "value": null,
            "status": "warning",
            "resolution": "missing",
            "message": "Agent Bridge registration does not define a command string."
        });
    };
    let path = Path::new(command);
    if path.is_absolute() {
        if path.exists() {
            json!({
                "value": command,
                "status": "ok",
                "resolution": "absolute_exists"
            })
        } else {
            json!({
                "value": command,
                "status": "warning",
                "resolution": "absolute_missing",
                "message": "Configured absolute command path does not exist."
            })
        }
    } else {
        json!({
            "value": command,
            "status": "info",
            "resolution": "path_lookup_required",
            "message": "Command is not absolute; the client PATH controls resolution."
        })
    }
}

pub(super) fn parse_json_registration(
    contents: &str,
) -> Result<Option<ClientRegistration>, String> {
    let value: Value = serde_json::from_str(contents).map_err(|error| error.to_string())?;
    let Some(servers) = value.get("mcpServers").and_then(Value::as_object) else {
        return Ok(None);
    };
    let similar_registrations = similar_json_registrations(servers);
    let Some(entry) = servers.get("agent-bridge").and_then(Value::as_object) else {
        return Ok(None);
    };
    Ok(Some(ClientRegistration {
        command: entry
            .get("command")
            .and_then(Value::as_str)
            .map(str::to_string),
        args: entry
            .get("args")
            .and_then(Value::as_array)
            .map(|values| string_array(values))
            .unwrap_or_default(),
        env_keys: entry
            .get("env")
            .and_then(Value::as_object)
            .map(|env| env.keys().cloned().collect())
            .unwrap_or_default(),
        similar_registrations,
    }))
}

fn similar_json_registrations(servers: &serde_json::Map<String, Value>) -> Vec<String> {
    servers
        .iter()
        .filter(|(name, _)| name.as_str() != "agent-bridge")
        .filter_map(|(name, value)| {
            value
                .get("command")
                .and_then(Value::as_str)
                .filter(|command| command.contains("agent-bridge-mcp"))
                .map(|_| name.clone())
        })
        .collect()
}

fn string_array(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

pub(super) fn parse_codex_registration(
    contents: &str,
) -> Result<Option<ClientRegistration>, String> {
    let mut current_section = String::new();
    let mut registration = ClientRegistration::default();
    let mut found = false;
    let mut similar = Vec::new();
    for line in contents.lines() {
        let line = strip_toml_comment(line).trim().to_string();
        if line.is_empty() {
            continue;
        }
        if let Some(section) = toml_section(&line) {
            current_section = section;
            continue;
        }
        let Some((key, value)) = toml_assignment(&line) else {
            continue;
        };
        if is_codex_agent_bridge_section(&current_section) {
            found = true;
            match key.as_str() {
                "command" => registration.command = parse_toml_string(&value),
                "args" => registration.args = parse_toml_string_array(&value),
                "env" => registration
                    .env_keys
                    .extend(parse_toml_inline_table_keys(&value)),
                _ => {}
            }
        } else if is_codex_agent_bridge_env_section(&current_section) {
            found = true;
            registration.env_keys.push(key);
        } else if is_codex_mcp_server_section(&current_section)
            && key == "command"
            && parse_toml_string(&value).is_some_and(|command| command.contains("agent-bridge-mcp"))
            && let Some(name) = codex_mcp_server_name(&current_section)
            && name != "agent-bridge"
        {
            similar.push(name);
        }
    }
    registration.similar_registrations = similar;
    if found {
        Ok(Some(registration))
    } else {
        Ok(None)
    }
}

fn strip_toml_comment(line: &str) -> String {
    let mut in_quote = false;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' if in_quote => escaped = true,
            '"' => in_quote = !in_quote,
            '#' if !in_quote => return line[..index].to_string(),
            _ => {}
        }
    }
    line.to_string()
}

fn toml_section(line: &str) -> Option<String> {
    line.strip_prefix('[')
        .and_then(|line| line.strip_suffix(']'))
        .map(|section| section.trim().to_string())
}

fn toml_assignment(line: &str) -> Option<(String, String)> {
    let (key, value) = line.split_once('=')?;
    Some((unquote_toml_key(key.trim()), value.trim().to_string()))
}

fn unquote_toml_key(key: &str) -> String {
    key.trim_matches('"').trim_matches('\'').to_string()
}

fn is_codex_agent_bridge_section(section: &str) -> bool {
    matches!(
        section,
        "mcp_servers.agent-bridge" | "mcp_servers.\"agent-bridge\"" | "mcp_servers.'agent-bridge'"
    )
}

fn is_codex_agent_bridge_env_section(section: &str) -> bool {
    matches!(
        section,
        "mcp_servers.agent-bridge.env"
            | "mcp_servers.\"agent-bridge\".env"
            | "mcp_servers.'agent-bridge'.env"
    )
}

fn is_codex_mcp_server_section(section: &str) -> bool {
    section.starts_with("mcp_servers.") && !section.ends_with(".env")
}

fn codex_mcp_server_name(section: &str) -> Option<String> {
    section
        .strip_prefix("mcp_servers.")
        .map(|name| unquote_toml_key(name.trim()))
}

fn parse_toml_string(value: &str) -> Option<String> {
    let value = value.trim();
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(|value| value.replace("\\\"", "\"").replace("\\\\", "\\"))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
                .map(str::to_string)
        })
}

fn parse_toml_string_array(value: &str) -> Vec<String> {
    let Some(inner) = value
        .trim()
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
    else {
        return Vec::new();
    };
    inner
        .split(',')
        .filter_map(|item| parse_toml_string(item.trim()))
        .collect()
}

fn parse_toml_inline_table_keys(value: &str) -> Vec<String> {
    let Some(inner) = value
        .trim()
        .strip_prefix('{')
        .and_then(|v| v.strip_suffix('}'))
    else {
        return Vec::new();
    };
    inner
        .split(',')
        .filter_map(|item| {
            item.split_once('=')
                .map(|(key, _)| unquote_toml_key(key.trim()))
        })
        .collect()
}

fn doctor_state() -> Value {
    let path = match crate::config::Config::from_env(crate::config::ConfigCliOverrides::default()) {
        Ok(config) => config.state_dir().to_path_buf(),
        Err(error) => {
            return json!({
                "status": "error",
                "path": null,
                "exists": false,
                "error": error
            });
        }
    };
    if let Err(error) = std::fs::create_dir_all(&path) {
        return json!({
            "status": "error",
            "path": path.display().to_string(),
            "exists": false,
            "error": error.to_string()
        });
    }
    match std::fs::metadata(&path) {
        Ok(metadata) if metadata.is_dir() => match doctor_registry_status(&path) {
            Ok(()) => json!({
                "status": "ok",
                "path": path.display().to_string(),
                "exists": true
            }),
            Err(error) => json!({
                "status": "error",
                "path": path.display().to_string(),
                "exists": true,
                "error": error
            }),
        },
        Ok(_) => json!({
            "status": "error",
            "path": path.display().to_string(),
            "exists": true,
            "error": "state path is not a directory"
        }),
        Err(error) => json!({
            "status": "error",
            "path": path.display().to_string(),
            "exists": false,
            "error": error.to_string()
        }),
    }
}

fn doctor_registry_status(state_dir: &Path) -> Result<(), String> {
    let registry_path = state_dir.join("registry.json");
    match std::fs::read_to_string(&registry_path) {
        Ok(contents) => validate_registry_text(&contents)
            .map_err(|error| format!("registry parse error: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("registry read error: {error}")),
    }
}

pub(super) fn doctor_orphans() -> Value {
    let Ok(config) = crate::config::Config::from_env(crate::config::ConfigCliOverrides::default())
    else {
        return json!({"status": "ok", "orphans": []});
    };
    let state_dir = config.state_dir().to_path_buf();
    let registry_path = state_dir.join("registry.json");
    let Ok(contents) = std::fs::read_to_string(&registry_path) else {
        return json!({"status": "ok", "orphans": []});
    };
    let Ok(mut value) = serde_json::from_str::<Value>(&contents) else {
        return json!({"status": "ok", "orphans": []});
    };
    // Normalize legacy fields so we can read agentId reliably.
    crate::task::normalize_legacy_registry_fields_exported(&mut value);
    let Some(tasks) = value.get("tasks").and_then(Value::as_object) else {
        return json!({"status": "ok", "orphans": []});
    };
    let mut orphan_list = Vec::new();
    for (id, task) in tasks {
        let status = task["status"].as_str().unwrap_or("");
        let worktree_managed = task["worktreeManaged"].as_bool().unwrap_or(false);
        let has_diagnostic = task.get("diagnostic").is_some() && !task["diagnostic"].is_null();
        // Orphan: FailedStale records that still track managed worktrees, or
        // any record with a worktree reclaim diagnostic.
        let is_stale_with_worktree = status == "failed_stale" && worktree_managed;
        let is_cleanup_failed = has_diagnostic
            && task
                .get("diagnostic")
                .and_then(|d| d.get("failureCategory"))
                .and_then(Value::as_str)
                .is_some_and(|cat| cat.contains("cleanup") || cat.contains("reclaim"));
        if is_stale_with_worktree || is_cleanup_failed {
            orphan_list.push(json!({
                "agentId": id,
                "status": status,
                "provider": task["provider"],
                "worktreeManaged": worktree_managed,
                "worktreePath": task.get("worktreePath").unwrap_or(&Value::Null),
                "diagnostic": task.get("diagnostic").unwrap_or(&Value::Null),
                "updatedAt": task.get("updatedAt").unwrap_or(&Value::Null),
            }));
        }
    }
    let status = if orphan_list.is_empty() {
        "ok"
    } else {
        "warning"
    };
    json!({
        "status": status,
        "orphans": orphan_list,
    })
}

async fn doctor_claude_host_runner() -> Value {
    let Some(socket_path) = crate::claude_host::socket_path_from_env() else {
        return json!({
            "status": "not_configured",
            "configured": false,
            "launchStrategy": "host_runner_required"
        });
    };
    let started = Instant::now();
    match timeout(
        Duration::from_millis(1_000),
        crate::claude_host::ping(&socket_path),
    )
    .await
    {
        Ok(Ok(response)) => doctor_claude_host_runner_response(
            &socket_path,
            started.elapsed().as_millis() as u64,
            response,
        ),
        Ok(Err(error)) => doctor_claude_host_runner_error_report(
            &socket_path,
            started.elapsed().as_millis() as u64,
            error,
        ),
        Err(_) => doctor_claude_host_runner_error_report(
            &socket_path,
            started.elapsed().as_millis() as u64,
            "host runner ping timed out after 1000ms".to_string(),
        ),
    }
}

pub(super) fn doctor_claude_host_runner_error_report(
    socket_path: &Path,
    ping_duration_ms: u64,
    error: String,
) -> Value {
    doctor_claude_host_runner_error_report_with(socket_path, ping_duration_ms, error, process_alive)
}

pub(super) fn doctor_claude_host_runner_error_report_with(
    socket_path: &Path,
    ping_duration_ms: u64,
    error: String,
    process_alive: impl Fn(u32) -> bool,
) -> Value {
    let mut report = json!({
        "status": "error",
        "configured": true,
        "launchStrategy": "host_runner",
        "socketPath": socket_path.display().to_string(),
        "pingDurationMs": ping_duration_ms,
        "error": error
    });
    if let Some((pid_path, pid)) = read_host_runner_pid(socket_path) {
        report["pidPath"] = json!(pid_path.display().to_string());
        report["pid"] = json!(pid);
        report["pidStatus"] = json!(if process_alive(pid) {
            "running"
        } else {
            "stale"
        });
    }
    report
}

fn read_host_runner_pid(socket_path: &Path) -> Option<(PathBuf, u32)> {
    let pid_path = socket_path.with_extension("pid");
    let pid = std::fs::read_to_string(&pid_path)
        .ok()?
        .trim()
        .parse::<u32>()
        .ok()?;
    Some((pid_path, pid))
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    pid > 0 && unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(not(unix))]
fn process_alive(_pid: u32) -> bool {
    false
}

pub(super) fn doctor_claude_host_runner_response(
    socket_path: &Path,
    ping_duration_ms: u64,
    response: crate::claude_host::HostResponse,
) -> Value {
    if response.ok {
        let mut report = json!({
            "status": "ok",
            "configured": true,
            "launchStrategy": "host_runner",
            "socketPath": socket_path.display().to_string(),
            "pingDurationMs": ping_duration_ms
        });
        if let Some(crate::claude_host::HostResult::Pong {
            protocol_version,
            workspace_policy_id,
            ready,
        }) = response.result
        {
            report["protocolVersion"] = json!(protocol_version);
            report["workspacePolicyId"] = json!(workspace_policy_id);
            report["ready"] = json!(ready);
        }
        return report;
    }
    json!({
        "status": "error",
        "configured": true,
        "launchStrategy": "host_runner",
        "socketPath": socket_path.display().to_string(),
        "pingDurationMs": ping_duration_ms,
        "errorCode": response.error.as_ref().map(|error| error.code.as_str()).unwrap_or("host_runner_error"),
        "error": response.error.map(|error| error.message).unwrap_or_else(|| "host runner ping failed".to_string())
    })
}

fn providers_status(providers: &Value) -> &'static str {
    let Some(providers) = providers.as_object() else {
        return "error";
    };
    if providers.values().any(|provider| {
        provider
            .get("available")
            .and_then(Value::as_bool)
            .is_some_and(|available| !available)
    }) {
        "warning"
    } else {
        "ok"
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn doctor_recommendations(
    workspace_status: &str,
    state_status: &str,
    provider_status: &str,
    host_status: &str,
    launch_readiness: &Value,
    clients: &Value,
    binary: &Value,
    orphans: &Value,
) -> Value {
    let mut recommendations = Vec::new();
    if workspace_status == "error" {
        recommendations.push(json!({
            "id": "configure_workspace",
            "severity": "error",
            "message": "Set AGENT_BRIDGE_WORKSPACES or pass a cwd inside a configured workspace.",
            "tool": "doctor",
            "arguments": {}
        }));
    }
    if state_status == "error" {
        recommendations.push(json!({
            "id": "fix_state_dir",
            "severity": "error",
            "message": "Fix AGENT_BRIDGE_STATE_DIR so Agent Bridge can read and write task state.",
            "tool": "doctor",
            "arguments": {}
        }));
    } else if state_status == "warning" {
        recommendations.push(json!({
            "id": "create_state_dir",
            "severity": "warning",
            "message": "Create AGENT_BRIDGE_STATE_DIR before spawning delegated tasks.",
            "tool": "doctor",
            "arguments": {}
        }));
    }
    if host_status == "error" {
        recommendations.push(json!({
            "id": "restart_claude_host_runner",
            "severity": "warning",
            "message": "Restart or reconfigure the Claude host runner with matching AGENT_BRIDGE_WORKSPACES, then rerun doctor.",
            "tool": "doctor",
            "arguments": {}
        }));
    }
    if provider_status == "warning" {
        recommendations.push(json!({
            "id": "fix_unavailable_providers",
            "severity": "warning",
            "message": "Install or configure unavailable providers, or pass providers to focus doctor output.",
            "tool": "doctor",
            "arguments": { "focus": "providers" }
        }));
    }
    let stale_providers: Vec<_> = launch_readiness["providers"]
        .as_object()
        .map(|providers| {
            providers
                .iter()
                .filter(|(_, provider)| {
                    provider["available"].as_bool() == Some(true)
                        && provider["startupVerified"].as_bool() == Some(false)
                })
                .map(|(name, _)| name.clone())
                .collect()
        })
        .unwrap_or_default();
    if !stale_providers.is_empty() {
        recommendations.push(json!({
            "id": "verify_provider_startup",
            "severity": "info",
            "message": "Selected providers are version-available but not startup-verified; run a bounded smoke check before first launch when startup readiness matters.",
            "tool": "doctor",
            "arguments": {
                "focus": "providers",
                "providers": stale_providers,
                "smoke": true
            }
        }));
    }
    recommendations.extend(client_recommendations(clients));
    recommendations.extend(binary_recommendations(binary));
    recommendations.extend(orphan_recommendations(orphans));
    Value::Array(recommendations)
}

fn binary_recommendations(binary: &Value) -> Vec<Value> {
    let mut recommendations = Vec::new();
    let status = binary["status"].as_str().unwrap_or("unknown");
    if status == "ok" {
        return recommendations;
    }
    if binary["release"]["exists"].as_bool() == Some(false) {
        recommendations.push(json!({
            "id": "build_release_binary",
            "severity": "info",
            "kind": "shell",
            "command": ["cargo", "build", "--release", "--bin", "agent-bridge-mcp"],
            "message": "Build the release Agent Bridge binary before comparing or installing binary freshness."
        }));
    }
    if binary["installed"]["exists"].as_bool() == Some(false)
        || binary["installed"]["matchesRelease"].as_bool() == Some(false)
    {
        let installed_path = binary["installed"]["path"]
            .as_str()
            .unwrap_or("~/.local/bin/agent-bridge-mcp");
        recommendations.push(json!({
            "id": "install_release_binary",
            "severity": "info",
            "kind": "shell",
            "command": ["install", "-m", "0755", "target/release/agent-bridge-mcp", installed_path],
            "message": "Install the release Agent Bridge binary after building it."
        }));
    }
    recommendations
}

pub(super) fn orphan_recommendations(orphans: &Value) -> Vec<Value> {
    let Some(orphan_list) = orphans["orphans"].as_array() else {
        return Vec::new();
    };
    if orphan_list.is_empty() {
        return Vec::new();
    }
    let agent_ids: Vec<&str> = orphan_list
        .iter()
        .filter_map(|o| o["agentId"].as_str())
        .collect();
    vec![json!({
        "id": "reclaim_orphaned_tasks",
        "severity": "warning",
        "message": format!(
            "Found {} orphaned task(s) with unreclaimed worktrees or failed cleanup: {}. Clean up the stale task state or rerun doctor after cleanup.",
            orphan_list.len(),
            agent_ids.join(", ")
        ),
        "tool": "doctor",
        "arguments": {}
    })]
}

fn client_recommendations(clients: &Value) -> Vec<Value> {
    let Some(clients) = clients.as_object() else {
        return Vec::new();
    };
    let mut recommendations = Vec::new();
    for (name, client) in clients {
        match client["registrationStatus"].as_str() {
            Some("registered") => {
                if let Some(command) = client["verificationCommands"]
                    .as_array()
                    .and_then(|commands| commands.first())
                    .and_then(|command| command["command"].as_array())
                {
                    recommendations.push(json!({
                        "id": format!("verify_{name}_client_config"),
                        "severity": "info",
                        "kind": "shell",
                        "command": command,
                        "message": format!("Run {} to verify the {name} client can load Agent Bridge.", command.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "))
                    }));
                }
            }
            Some("absent") if matches!(client["parseStatus"].as_str(), Some("ok" | "missing")) => {
                recommendations.push(json!({
                    "id": format!("configure_{name}_client"),
                    "severity": "info",
                    "message": format!("Add Agent Bridge to the {name} user-level MCP config only if you use that client.")
                }));
            }
            _ if client["parseStatus"].as_str() == Some("error") => {
                recommendations.push(json!({
                    "id": format!("fix_{name}_client_config"),
                    "severity": "warning",
                    "message": format!("Fix the {name} MCP config parse/read error before relying on that client.")
                }));
            }
            _ => {}
        }
    }
    recommendations
}

fn doctor_launch_readiness(providers: &Value, selected: Option<&[ProviderKind]>) -> Value {
    let mut provider_readiness = serde_json::Map::new();
    let mut any_not_verified = false;
    let mut all_launchable = true;
    if let Some(providers) = providers.as_object() {
        for (name, provider) in providers {
            let available = provider["available"].as_bool().unwrap_or(false);
            let startup_verified = provider["startupVerified"].as_bool().unwrap_or(false);
            let launchable = provider["launchable"].as_bool().unwrap_or(false);
            any_not_verified |= available && !startup_verified;
            all_launchable &= launchable;
            provider_readiness.insert(
                name.clone(),
                json!({
                    "available": available,
                    "startupVerified": startup_verified,
                    "launchable": launchable,
                    "readiness": provider.get("readiness").cloned().unwrap_or_else(|| json!({}))
                }),
            );
        }
    }
    let selected_providers = selected
        .map(|providers| {
            providers
                .iter()
                .map(|provider| provider.as_str())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "status": if all_launchable {
            "ready"
        } else if any_not_verified {
            "not_verified"
        } else {
            "not_launchable"
        },
        "startupVerified": !any_not_verified && all_launchable,
        "launchable": all_launchable,
        "selectedProviders": selected_providers,
        "providers": provider_readiness
    })
}

fn aggregate_status<'a>(statuses: impl IntoIterator<Item = &'a str>) -> &'static str {
    let mut aggregate = "ok";
    for status in statuses {
        match status {
            "error" => return "error",
            "warning" if aggregate == "ok" => aggregate = "warning",
            _ => {}
        }
    }
    aggregate
}

async fn providers_check(arguments: Value) -> Result<Value, String> {
    let input: ProvidersCheckInput =
        serde_json::from_value(arguments).map_err(|error| error.to_string())?;
    let selected = selected_providers(input.providers.as_deref())?;
    validate_provider_budgets(&input)?;
    let aggregate_timeout_ms = aggregate_timeout_ms(input.aggregate_timeout_ms)?;
    let profile = input.profile.unwrap_or(LaunchProfile::Bridge);
    let smoke_cwd = if input.smoke {
        Some(match input.cwd.as_deref() {
            Some(cwd) => safe_cwd(Some(cwd))?,
            None => default_cwd(),
        })
    } else {
        None
    };
    let mut results = serde_json::Map::new();
    let mut smoke_candidates = Vec::new();
    for provider in selected.iter().copied() {
        let command = match provider::version_command(provider) {
            Ok(command) => command,
            Err(error) => {
                let checked_at = checked_at_iso();
                let diagnostic = json!({
                    "failureCategory": FailureCategory::ProviderStartError.as_str(),
                    "provider": provider.as_str(),
                    "commandKind": "acp",
                    "commandPath": null,
                    "launchStrategy": "acp",
                    "startupVerified": false,
                    "timeoutMs": VERSION_TIMEOUT_MS,
                    "elapsedMs": 0,
                    "phase": "version",
                    "error": error,
                });
                let mut value = json!({
                    "available": false,
                    "command": null,
                    "probe": "version",
                    "startupVerified": false,
                    "launchable": false,
                    "checkedAt": checked_at,
                    "error": diagnostic["error"],
                    "versionDurationMs": 0,
                    "diagnostic": diagnostic
                });
                let diagnostic = value["diagnostic"].clone();
                set_readiness(
                    &mut value,
                    "failed",
                    "version",
                    false,
                    false,
                    Some(diagnostic),
                );
                results.insert(provider.as_str().to_string(), value);
                continue;
            }
        };
        let output = run_probe(&command, provider, VERSION_TIMEOUT_MS, "version").await;
        let checked_at = checked_at_iso();
        let value = if output.success {
            let mut value = json!({
                "available": true,
                "command": command.command.clone(),
                "version": String::from_utf8_lossy(&output.stdout).trim(),
                "probe": if input.smoke { "version+smoke" } else { "version" },
                "startupVerified": false,
                "launchable": false,
                "checkedAt": checked_at,
                "versionDurationMs": output.duration_ms
            });
            value["profile"] = json!(profile);
            set_readiness(&mut value, "stale", "version", false, false, None);
            value
        } else {
            let diagnostic = provider_diagnostic(
                provider,
                &command,
                &output,
                VERSION_TIMEOUT_MS,
                false,
                "version",
            );
            let mut value = json!({
                "available": false,
                "command": command.command.clone(),
                "probe": "version",
                "startupVerified": false,
                "launchable": false,
                "checkedAt": checked_at,
                "error": probe_error_text(&output),
                "versionDurationMs": output.duration_ms,
                "diagnostic": diagnostic
            });
            let diagnostic = value["diagnostic"].clone();
            set_readiness(
                &mut value,
                "failed",
                "version",
                false,
                false,
                Some(diagnostic),
            );
            value
        };
        if input.smoke && value["available"].as_bool() == Some(true) {
            smoke_candidates.push((provider, value));
        } else {
            results.insert(provider.as_str().to_string(), value);
        }
    }
    if input.smoke {
        let smoked = run_smoke_checks(
            smoke_candidates,
            &input,
            aggregate_timeout_ms,
            smoke_cwd.unwrap_or_else(default_cwd),
            profile,
        )
        .await;
        for (provider, value) in smoked {
            results.insert(provider.as_str().to_string(), value);
        }
    }
    Ok(json!({ "providers": results }))
}

fn selected_providers(input: Option<&[ProviderKind]>) -> Result<Vec<ProviderKind>, String> {
    let Some(input) = input else {
        return Ok(ProviderKind::ALL.to_vec());
    };
    if input.is_empty() {
        return Err("providers must select at least one provider".to_string());
    }
    let mut selected = Vec::new();
    for provider in input {
        if !selected.contains(provider) {
            selected.push(*provider);
        }
    }
    Ok(selected)
}

fn validate_provider_budgets(input: &ProvidersCheckInput) -> Result<(), String> {
    let Some(provider_timeout_ms) = input.provider_timeout_ms.as_ref() else {
        return Ok(());
    };
    for (provider, value) in provider_timeout_ms {
        provider
            .parse::<ProviderKind>()
            .map_err(|error| format!("providerTimeoutMs.{provider}: {error}"))?;
        validate_timeout_range(
            *value,
            MAX_PROVIDER_TIMEOUT_MS,
            &format!("providerTimeoutMs.{provider}"),
        )?;
    }
    Ok(())
}

fn aggregate_timeout_ms(value: Option<i64>) -> Result<u64, String> {
    match value {
        Some(value) => {
            validate_timeout_range(value, MAX_AGGREGATE_TIMEOUT_MS, "aggregateTimeoutMs")
        }
        None => Ok(DEFAULT_AGGREGATE_TIMEOUT_MS),
    }
}

fn validate_timeout_range(value: i64, max: i64, field: &str) -> Result<u64, String> {
    if !(1..=max).contains(&value) {
        return Err(format!("{field} must be an integer from 1 through {max}"));
    }
    Ok(value as u64)
}

fn provider_smoke_timeout_ms(provider: ProviderKind, input: &ProvidersCheckInput) -> u64 {
    input
        .provider_timeout_ms
        .as_ref()
        .and_then(|timeouts| timeouts.get(provider.as_str()))
        .copied()
        .or(input.timeout_ms)
        .map(|value| value.clamp(1, MAX_PROVIDER_TIMEOUT_MS) as u64)
        .unwrap_or_else(|| default_provider_smoke_timeout_ms(provider))
}

fn default_provider_smoke_timeout_ms(provider: ProviderKind) -> u64 {
    match provider {
        ProviderKind::Codex => 20_000,
        ProviderKind::Forge => 60_000,
        ProviderKind::Claude => 60_000,
        ProviderKind::Kimi => 45_000,
        ProviderKind::Cursor => 60_000,
        ProviderKind::Antigravity => 60_000,
    }
}

fn smoke_concurrency() -> usize {
    env::var("AGENT_BRIDGE_SMOKE_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .map(|value| value.min(4))
        .unwrap_or(2)
}

async fn run_smoke_checks(
    candidates: Vec<(ProviderKind, Value)>,
    input: &ProvidersCheckInput,
    aggregate_timeout_ms: u64,
    cwd: String,
    profile: LaunchProfile,
) -> Vec<(ProviderKind, Value)> {
    let order: Vec<ProviderKind> = candidates.iter().map(|(provider, _)| *provider).collect();
    let deadline = Instant::now() + Duration::from_millis(aggregate_timeout_ms);
    let mut pending: VecDeque<_> = candidates.into();
    let mut running = JoinSet::new();
    let mut results = Vec::new();
    let concurrency = smoke_concurrency();
    loop {
        while running.len() < concurrency && !pending.is_empty() {
            let remaining_ms = deadline
                .checked_duration_since(Instant::now())
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or(0);
            if remaining_ms == 0 {
                break;
            }
            let (provider, base_value) = pending.pop_front().unwrap();
            let provider_timeout_ms = provider_smoke_timeout_ms(provider, input);
            let timeout_ms = provider_timeout_ms.min(remaining_ms);
            let cwd = cwd.clone();
            running.spawn(async move {
                smoke_one_provider(provider, base_value, timeout_ms, cwd, profile).await
            });
        }
        if running.is_empty() {
            break;
        }
        match running.join_next().await {
            Some(Ok(result)) => results.push(result),
            Some(Err(error)) => {
                tracing::error!(error = %error, "[agent-bridge] smoke task join error: {error}");
            }
            None => break,
        }
    }
    for (provider, mut value) in pending {
        if provider != ProviderKind::Antigravity {
            value["available"] = json!(false);
        }
        value["startupVerified"] = json!(false);
        value["launchable"] = json!(false);
        value["checkedAt"] = json!(checked_at_iso());
        value["error"] =
            json!("aggregate provider readiness timeout expired before smoke probe started");
        value["diagnostic"] = json!({
            "failureCategory": "provider_timeout",
            "provider": provider.as_str(),
            "startupVerified": false,
            "timeoutMs": aggregate_timeout_ms,
            "phase": "smoke"
        });
        let diagnostic = value["diagnostic"].clone();
        set_readiness(
            &mut value,
            "failed",
            "smoke",
            false,
            false,
            Some(diagnostic),
        );
        results.push((provider, value));
    }
    results.sort_by_key(|(provider, _)| {
        order
            .iter()
            .position(|candidate| candidate == provider)
            .unwrap_or(usize::MAX)
    });
    results
}

async fn smoke_one_provider(
    provider: ProviderKind,
    base_value: Value,
    timeout_ms: u64,
    cwd: String,
    profile: LaunchProfile,
) -> (ProviderKind, Value) {
    let smoke_value =
        match provider::smoke_command(provider, &cwd, (timeout_ms / 1000).max(1) as i64, profile) {
            Ok((smoke_command, strategy)) => {
                let mut output = run_probe(&smoke_command, provider, timeout_ms, "smoke").await;
                if output.success
                    && output.failure_category.is_none()
                    && !smoke_output_is_accepted(provider, &output.stdout)
                {
                    output.failure_category = Some(FailureCategory::ProviderOutputError);
                    output.error =
                        Some("provider smoke output did not contain expected token".to_string());
                }
                if output.success && output.failure_category.is_none() {
                    let mut value = base_value;
                    value["startupVerified"] = json!(true);
                    value["launchable"] = json!(true);
                    value["checkedAt"] = json!(checked_at_iso());
                    value["smokeDurationMs"] = json!(output.duration_ms);
                    value["smokePromptStrategy"] = json!(strategy);
                    value["launchStrategy"] = json!(launch_strategy(&smoke_command));
                    set_readiness(&mut value, "ready", "version+smoke", true, true, None);
                    value
                } else {
                    let mut value = base_value;
                    if provider != ProviderKind::Antigravity {
                        value["available"] = json!(false);
                    }
                    value["startupVerified"] = json!(false);
                    value["launchable"] = json!(false);
                    value["checkedAt"] = json!(checked_at_iso());
                    value["smokeDurationMs"] = json!(output.duration_ms);
                    value["smokePromptStrategy"] = json!(strategy);
                    value["launchStrategy"] = json!(launch_strategy(&smoke_command));
                    value["error"] = json!(probe_error_text(&output));
                    value["diagnostic"] = provider_diagnostic(
                        provider,
                        &smoke_command,
                        &output,
                        timeout_ms,
                        false,
                        "smoke",
                    );
                    let diagnostic = value["diagnostic"].clone();
                    set_readiness(
                        &mut value,
                        "failed",
                        "version+smoke",
                        false,
                        false,
                        Some(diagnostic),
                    );
                    value
                }
            }
            Err(error) => {
                let mut value = base_value;
                value["available"] = json!(false);
                value["startupVerified"] = json!(false);
                value["launchable"] = json!(false);
                value["checkedAt"] = json!(checked_at_iso());
                value["error"] = json!(error);
                set_readiness(&mut value, "failed", "version+smoke", false, false, None);
                value
            }
        };
    (provider, smoke_value)
}

fn set_readiness(
    value: &mut Value,
    state: &'static str,
    probe: &'static str,
    startup_verified: bool,
    launchable: bool,
    diagnostic: Option<Value>,
) {
    let mut readiness = json!({
        "state": state,
        "startupVerified": startup_verified,
        "launchable": launchable,
        "probe": probe,
        "checkedAt": value["checkedAt"],
        "versionDurationMs": value["versionDurationMs"]
    });
    if value.get("smokeDurationMs").is_some() {
        readiness["smokeDurationMs"] = value["smokeDurationMs"].clone();
    }
    if let Some(diagnostic) = diagnostic {
        readiness["diagnostic"] = diagnostic;
    }
    value["readiness"] = readiness;
}

fn checked_at_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
