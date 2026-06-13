use super::Registry;
use chrono::Utc;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs as std_fs;
use std::io;
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::path::Path;
use tokio::fs;
use uuid::Uuid;
pub(super) fn cap_string(value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        value
    } else {
        String::from_utf8_lossy(&value.as_bytes()[..max_bytes]).to_string()
    }
}

pub(super) async fn load_registry(state_dir: &Path) -> Result<Registry, String> {
    cleanup_registry_temps(state_dir).await?;
    let path = state_dir.join("registry.json");
    match fs::read_to_string(&path).await {
        Ok(text) => parse_registry_text(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Registry {
            tasks: BTreeMap::new(),
        }),
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) fn validate_registry_text(text: &str) -> Result<(), String> {
    parse_registry_text(text).map(|_| ())
}

pub(super) fn parse_registry_text(text: &str) -> Result<Registry, String> {
    let mut value: Value = serde_json::from_str(text)
        .map_err(|error| format!("failed to parse registry.json: {error}"))?;
    normalize_legacy_registry_fields(&mut value);
    serde_json::from_value(value).map_err(|error| format!("failed to parse registry.json: {error}"))
}

pub(crate) fn normalize_legacy_registry_fields_exported(value: &mut Value) {
    normalize_legacy_registry_fields(value)
}

pub(super) fn normalize_legacy_registry_fields(value: &mut Value) {
    let Some(tasks) = value.get_mut("tasks").and_then(Value::as_object_mut) else {
        return;
    };
    for task in tasks.values_mut() {
        let Some(record) = task.as_object_mut() else {
            continue;
        };
        if !record.contains_key("agentId")
            && let Some(task_id) = record.get("taskId").cloned()
        {
            record.insert("agentId".to_string(), task_id);
        }
        if !record.contains_key("agentDir")
            && let Some(task_dir) = record.get("taskDir").cloned()
        {
            record.insert("agentDir".to_string(), task_dir);
        }
    }
}

pub(super) async fn cleanup_registry_temps(state_dir: &Path) -> Result<(), String> {
    let mut entries = match fs::read_dir(state_dir).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| error.to_string())?
    {
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with("registry.json.tmp-")
        {
            let _ = fs::remove_file(entry.path()).await;
        }
    }
    Ok(())
}

pub(super) async fn save_registry(state_dir: &Path, registry: &Registry) -> Result<(), String> {
    let state_dir = state_dir.to_path_buf();
    let registry = registry.clone();
    tokio::task::spawn_blocking(move || save_registry_blocking(&state_dir, &registry))
        .await
        .map_err(|error| error.to_string())?
}

fn save_registry_blocking(state_dir: &Path, registry: &Registry) -> Result<(), String> {
    std_fs::create_dir_all(state_dir).map_err(|error| error.to_string())?;
    let registry_path = state_dir.join("registry.json");
    let lock_path = state_dir.join("registry.lock");
    let _lock = RegistryLock::acquire(&lock_path)?;
    let mut merged = match std_fs::read_to_string(&registry_path) {
        Ok(text) => parse_registry_text(&text)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Registry {
            tasks: BTreeMap::new(),
        },
        Err(error) => return Err(error.to_string()),
    };
    merge_registry(&mut merged, registry);
    let tmp_path = state_dir.join(format!(
        "registry.json.tmp-{}-{}",
        std::process::id(),
        Uuid::new_v4().simple()
    ));
    let bytes = serde_json::to_vec_pretty(&merged).map_err(|error| error.to_string())?;
    std_fs::write(&tmp_path, bytes).map_err(|error| error.to_string())?;
    std_fs::rename(&tmp_path, &registry_path).map_err(|error| error.to_string())
}

pub(super) fn merge_registry(target: &mut Registry, source: &Registry) {
    for (agent_id, incoming) in &source.tasks {
        let replace = target
            .tasks
            .get(agent_id)
            .is_none_or(|existing| incoming.updated_at >= existing.updated_at);
        if replace {
            target.tasks.insert(agent_id.clone(), incoming.clone());
        }
    }
}

struct RegistryLock {
    file: std_fs::File,
}

impl RegistryLock {
    fn acquire(path: &Path) -> Result<Self, String> {
        let file = std_fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)
            .map_err(|error| error.to_string())?;
        lock_file(&file)?;
        Ok(Self { file })
    }
}

impl Drop for RegistryLock {
    fn drop(&mut self) {
        let _ = unlock_file(&self.file);
    }
}

#[cfg(unix)]
fn lock_file(file: &std_fs::File) -> Result<(), String> {
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error().to_string())
    }
}

#[cfg(unix)]
fn unlock_file(file: &std_fs::File) -> Result<(), String> {
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error().to_string())
    }
}

#[cfg(not(unix))]
fn lock_file(_file: &std_fs::File) -> Result<(), String> {
    Ok(())
}

#[cfg(not(unix))]
fn unlock_file(_file: &std_fs::File) -> Result<(), String> {
    Ok(())
}

pub(super) fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
