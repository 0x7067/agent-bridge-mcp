use serde_json::{Value, json};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const HOOK_TIMEOUT_SECONDS: u64 = 5;
const MAX_HOOK_EVENT_LINE_BYTES: usize = 1024 * 1024;
pub const HOOK_EVENT_LOG_ENV: &str = "AGENT_BRIDGE_CLAUDE_HOOK_EVENT_LOG";
pub const HOOK_RUN_ID_ENV: &str = "AGENT_BRIDGE_CLAUDE_RUN_ID";

pub struct HookSettings {
    pub settings_path: PathBuf,
}

pub struct HookRelay {
    pub run_dir: PathBuf,
    pub event_log_path: PathBuf,
    pub helper_path: PathBuf,
    pub run_id: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct HookRelayEvent {
    pub event_name: String,
    pub payload: Value,
}

impl HookRelay {
    pub fn prepare(run_dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(run_dir)?;
        set_owner_only_dir(run_dir)?;
        let event_log_path = run_dir.join("events.log");
        let helper_path = run_dir.join("hook-relay");
        create_event_log(&event_log_path)?;
        write_hook_helper(&helper_path)?;
        Ok(Self {
            run_dir: run_dir.to_path_buf(),
            event_log_path,
            helper_path,
            run_id: Uuid::new_v4().to_string(),
        })
    }

    pub fn open_reader(&self) -> io::Result<File> {
        OpenOptions::new().read(true).open(&self.event_log_path)
    }

    pub fn env(&self) -> [(String, String); 2] {
        [
            (
                HOOK_EVENT_LOG_ENV.to_string(),
                self.event_log_path.display().to_string(),
            ),
            (HOOK_RUN_ID_ENV.to_string(), self.run_id.clone()),
        ]
    }

    pub fn cleanup(self) -> io::Result<()> {
        fs::remove_dir_all(self.run_dir)
    }
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

pub fn parse_event_line(line: &[u8]) -> io::Result<HookRelayEvent> {
    if line.len() > MAX_HOOK_EVENT_LINE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "hook relay event line exceeded limit",
        ));
    }
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let Some(tab_index) = line.iter().position(|byte| *byte == b'\t') else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "hook relay event missing separator",
        ));
    };
    let event_name = std::str::from_utf8(&line[..tab_index])
        .map_err(io::Error::other)?
        .to_string();
    if !matches!(event_name.as_str(), "SessionStart" | "Stop" | "StopFailure") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "hook relay event name is not supported",
        ));
    }
    let payload = serde_json::from_slice(&line[tab_index + 1..]).map_err(io::Error::other)?;
    Ok(HookRelayEvent {
        event_name,
        payload,
    })
}

fn create_event_log(path: &Path) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    options.open(path)?;
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(io::Error::other("hook relay path is not a regular file"));
    }
    #[cfg(unix)]
    {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
    }
    #[cfg(not(unix))]
    {
        Ok(())
    }
}

fn write_hook_helper(path: &Path) -> io::Result<()> {
    let script = format!(
        r#"#!/bin/sh
set -eu
event="${{1:?event}}"
event_log="${{{event_log_env}:?event_log}}"
if [ "$event" = "Stop" ]; then
  payload='{{"hook_event_name":"Stop"}}'
else
  payload="$(cat)"
fi
printf '%s\t%s\n' "$event" "$payload" >> "$event_log"
"#,
        event_log_env = HOOK_EVENT_LOG_ENV
    );
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o700);
    let mut file = options.open(path)?;
    file.write_all(script.as_bytes())?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
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
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
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

    #[test]
    #[cfg(unix)]
    fn hook_relay_helper_writes_event_log_events_without_stdout() {
        let run_dir = temp_path("relay");
        let relay = HookRelay::prepare(&run_dir).unwrap();
        let mut reader = relay.open_reader().unwrap();
        let mut command = Command::new(&relay.helper_path)
            .arg("Stop")
            .envs(relay.env())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        command
            .stdin
            .as_mut()
            .unwrap()
            .write_all(br#"{"hook_event_name":"Stop","transcript_path":"/tmp/t.jsonl"}"#)
            .unwrap();
        drop(command.stdin.take());
        let output = command.wait_with_output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(output.stdout.is_empty());

        let line = read_relay_line(&mut reader, Duration::from_secs(2)).unwrap();
        let event = parse_event_line(&line).unwrap();
        assert_eq!(event.event_name, "Stop");
        assert_eq!(event.payload["hook_event_name"], "Stop");
        assert!(event.payload.get("transcript_path").is_none());

        let event_log_mode = fs::metadata(&relay.event_log_path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        let helper_mode = fs::metadata(&relay.helper_path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(event_log_mode, 0o600);
        assert_eq!(helper_mode, 0o700);
        relay.cleanup().unwrap();
        assert!(!run_dir.exists());
    }

    #[test]
    fn event_parser_rejects_unknown_events() {
        let error = parse_event_line(br#"Unexpected	{"ok":true}"#).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[cfg(unix)]
    fn read_relay_line(reader: &mut File, duration: Duration) -> io::Result<Vec<u8>> {
        let started = Instant::now();
        let mut line = Vec::new();
        let mut byte = [0_u8; 1];
        while started.elapsed() < duration {
            match reader.read(&mut byte) {
                Ok(0) => std::thread::sleep(Duration::from_millis(10)),
                Ok(_) => {
                    line.push(byte[0]);
                    if byte[0] == b'\n' {
                        return Ok(line);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "timed out waiting for hook relay line",
        ))
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
