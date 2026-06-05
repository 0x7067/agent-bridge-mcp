use serde_json::{Value, json};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

const HOOK_TIMEOUT_SECONDS: u64 = 5;

pub struct HookSettings {
    pub settings_path: PathBuf,
}

pub fn write_temporary_settings(run_dir: &Path, hook_relay: &Path) -> io::Result<HookSettings> {
    fs::create_dir_all(run_dir)?;
    set_owner_only_dir(run_dir)?;
    let settings_path = run_dir.join("settings.json");
    let settings = settings_json(hook_relay);
    write_owner_only_json(&settings_path, &settings)?;
    Ok(HookSettings { settings_path })
}

pub fn settings_json(hook_relay: &Path) -> Value {
    json!({
        "hooks": {
            "SessionStart": [
                hook_group(Some("startup|resume|clear"), hook_relay, "SessionStart")
            ],
            "Stop": [
                hook_group(None, hook_relay, "Stop")
            ],
            "StopFailure": [
                hook_group(None, hook_relay, "StopFailure")
            ]
        }
    })
}

fn hook_group(matcher: Option<&str>, hook_relay: &Path, event: &str) -> Value {
    let mut group = json!({
        "hooks": [
            {
                "type": "command",
                "command": hook_relay.display().to_string(),
                "args": [event],
                "timeout": HOOK_TIMEOUT_SECONDS
            }
        ]
    });
    if let Some(matcher) = matcher {
        group["matcher"] = json!(matcher);
    }
    group
}

fn write_owner_only_json(path: &Path, value: &Value) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(unix)]
fn set_owner_only_dir(path: &Path) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_owner_only_dir(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn settings_json_registers_runner_owned_lifecycle_hooks() {
        let settings = settings_json(Path::new("/tmp/agent-bridge/hook-relay"));
        let hooks = &settings["hooks"];
        for event in ["SessionStart", "Stop", "StopFailure"] {
            let group = &hooks[event][0];
            let command = &group["hooks"][0];
            assert_eq!(command["type"], "command");
            assert_eq!(command["command"], "/tmp/agent-bridge/hook-relay");
            assert_eq!(command["args"], json!([event]));
            assert_eq!(command["timeout"], HOOK_TIMEOUT_SECONDS);
        }
        assert_eq!(hooks["SessionStart"][0]["matcher"], "startup|resume|clear");
        assert!(hooks["Stop"][0].get("matcher").is_none());
        assert!(hooks["StopFailure"][0].get("matcher").is_none());
    }

    #[test]
    #[cfg(unix)]
    fn temporary_settings_are_owner_only_and_not_durable_config() {
        let run_dir = temp_path("settings");
        let hook_relay = run_dir.join("hook-relay");
        let settings = write_temporary_settings(&run_dir, &hook_relay).unwrap();
        assert_eq!(settings.settings_path, run_dir.join("settings.json"));
        assert!(settings.settings_path.starts_with(&run_dir));

        let dir_mode = fs::metadata(&run_dir).unwrap().permissions().mode() & 0o777;
        let file_mode = fs::metadata(&settings.settings_path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700);
        assert_eq!(file_mode, 0o600);

        let parsed: Value =
            serde_json::from_slice(&fs::read(&settings.settings_path).unwrap()).unwrap();
        assert_eq!(
            parsed["hooks"]["StopFailure"][0]["hooks"][0]["args"],
            json!(["StopFailure"])
        );
    }

    fn temp_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "agent-bridge-hook-{label}-{}-{nonce}",
            std::process::id()
        ))
    }
}
