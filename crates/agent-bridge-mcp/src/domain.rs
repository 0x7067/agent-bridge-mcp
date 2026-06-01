use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

pub const DEFAULT_TIMEOUT_SECONDS: i64 = 120;
pub const MAX_TIMEOUT_SECONDS: i64 = 1800;
pub const MIN_TIMEOUT_SECONDS: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    #[serde(rename = "claude")]
    Claude,
    #[serde(rename = "cursor")]
    Cursor,
    #[serde(rename = "kimi")]
    Kimi,
    #[serde(rename = "codex")]
    Codex,
}

impl ProviderKind {
    pub const ALL: [Self; 4] = [Self::Claude, Self::Cursor, Self::Kimi, Self::Codex];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Cursor => "cursor",
            Self::Kimi => "kimi",
            Self::Codex => "codex",
        }
    }
}

impl FromStr for ProviderKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "claude" => Ok(Self::Claude),
            "cursor" => Ok(Self::Cursor),
            "kimi" => Ok(Self::Kimi),
            "codex" => Ok(Self::Codex),
            _ => Err(format!(
                "provider must be one of: {}",
                join_provider_names()
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskMode {
    #[serde(rename = "research")]
    Research,
    #[serde(rename = "review")]
    Review,
    #[serde(rename = "implement")]
    Implement,
    #[serde(rename = "command")]
    Command,
}

impl TaskMode {
    pub const ALL: [Self; 4] = [Self::Research, Self::Review, Self::Implement, Self::Command];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::Review => "review",
            Self::Implement => "implement",
            Self::Command => "command",
        }
    }
}

impl FromStr for TaskMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "research" => Ok(Self::Research),
            "review" => Ok(Self::Review),
            "implement" => Ok(Self::Implement),
            "command" => Ok(Self::Command),
            _ => Err(format!("mode must be one of: {}", join_task_modes())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Stopped,
    FailedStale,
    Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPhase {
    Pending,
    Active,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    Timeout,
    ProviderExitError,
    ProviderStartError,
    Stopped,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Isolation {
    None,
    Worktree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeoutSeconds(i64);

impl TimeoutSeconds {
    pub fn new(value: Option<i64>) -> Self {
        let numeric = value.unwrap_or(DEFAULT_TIMEOUT_SECONDS);
        Self(numeric.clamp(MIN_TIMEOUT_SECONDS, MAX_TIMEOUT_SECONDS))
    }

    pub fn get(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeName(String);

impl WorktreeName {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '.' | '_' | '-'))
        {
            Ok(Self(value))
        } else {
            Err(
                "worktreeName may contain only letters, numbers, dot, underscore, and hyphen"
                    .to_string(),
            )
        }
    }
}

impl fmt::Display for WorktreeName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub fn provider_names() -> Vec<&'static str> {
    ProviderKind::ALL
        .iter()
        .map(|provider| provider.as_str())
        .collect()
}

pub fn task_modes() -> Vec<&'static str> {
    TaskMode::ALL.iter().map(|mode| mode.as_str()).collect()
}

fn join_provider_names() -> String {
    provider_names().join(", ")
}

fn join_task_modes() -> String {
    task_modes().join(", ")
}
