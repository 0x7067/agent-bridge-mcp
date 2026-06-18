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
    #[serde(rename = "forge")]
    Forge,
    #[serde(rename = "antigravity")]
    Antigravity,
}

impl ProviderKind {
    pub const ALL: [Self; 6] = [
        Self::Claude,
        Self::Cursor,
        Self::Kimi,
        Self::Codex,
        Self::Forge,
        Self::Antigravity,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Cursor => "cursor",
            Self::Kimi => "kimi",
            Self::Codex => "codex",
            Self::Forge => "forge",
            Self::Antigravity => "antigravity",
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
            "forge" => Ok(Self::Forge),
            "antigravity" => Ok(Self::Antigravity),
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
    ProviderOutputError,
    CodexSandboxDenied,
    Stopped,
    Stale,
}

/// Bounded retry policy attached to a spawned task. Evaluated by the actor
/// when a completion carries a transient failure category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryPolicy {
    #[serde(rename = "maxRetries")]
    pub max_retries: u32,
    #[serde(rename = "backoffMs")]
    pub backoff_ms: u64,
}

/// An individual partial result extracted from the transcript tail when a
/// provider emits output events without ever producing a final `provider_result`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialResult {
    pub timestamp: String,
    pub source: String,
    pub kind: String,
    pub summary: String,
}

/// Strongly typed failure categories used across provider probes, task
/// lifecycle diagnostics, and the host-runner wire format. Serialized as
/// snake_case at the JSON boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureCategory {
    ProviderTimeout,
    ProviderOutputError,
    ProviderExitError,
    ProviderStartError,
    ProviderSandboxDenied,
    HostRunnerUnavailable,
    WorktreeCleanupFailed,
    WorktreeReclaimFailed,
    AgentDirCleanupFailed,
    TranscriptUnavailable,
    ClaudeApiError,
    ClaudeAuthError,
    ClaudeBillingError,
    ClaudeRateLimit,
    ClaudeModelUnavailable,
    ClaudeSetupRequired,
    RunnerTimeout,
    ClientDisconnected,
}

impl FailureCategory {
    /// Returns true for categories eligible for the automatic retry policy.
    pub fn is_transient(self) -> bool {
        matches!(
            self,
            Self::ProviderTimeout | Self::ProviderStartError | Self::HostRunnerUnavailable
        )
    }

    /// Returns the snake_case string used at the JSON boundary.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProviderTimeout => "provider_timeout",
            Self::ProviderOutputError => "provider_output_error",
            Self::ProviderExitError => "provider_exit_error",
            Self::ProviderStartError => "provider_start_error",
            Self::ProviderSandboxDenied => "provider_sandbox_denied",
            Self::HostRunnerUnavailable => "host_runner_unavailable",
            Self::WorktreeCleanupFailed => "worktree_cleanup_failed",
            Self::WorktreeReclaimFailed => "worktree_reclaim_failed",
            Self::AgentDirCleanupFailed => "agent_dir_cleanup_failed",
            Self::TranscriptUnavailable => "transcript_unavailable",
            Self::ClaudeApiError => "claude_api_error",
            Self::ClaudeAuthError => "claude_auth_error",
            Self::ClaudeBillingError => "claude_billing_error",
            Self::ClaudeRateLimit => "claude_rate_limit",
            Self::ClaudeModelUnavailable => "claude_model_unavailable",
            Self::ClaudeSetupRequired => "claude_setup_required",
            Self::RunnerTimeout => "runner_timeout",
            Self::ClientDisconnected => "client_disconnected",
        }
    }
}

impl fmt::Display for FailureCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for FailureCategory {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "provider_timeout" => Ok(Self::ProviderTimeout),
            "provider_output_error" => Ok(Self::ProviderOutputError),
            "provider_exit_error" => Ok(Self::ProviderExitError),
            "provider_start_error" => Ok(Self::ProviderStartError),
            "provider_sandbox_denied" => Ok(Self::ProviderSandboxDenied),
            "host_runner_unavailable" => Ok(Self::HostRunnerUnavailable),
            "worktree_cleanup_failed" => Ok(Self::WorktreeCleanupFailed),
            "worktree_reclaim_failed" => Ok(Self::WorktreeReclaimFailed),
            "agent_dir_cleanup_failed" => Ok(Self::AgentDirCleanupFailed),
            "transcript_unavailable" => Ok(Self::TranscriptUnavailable),
            "claude_api_error" => Ok(Self::ClaudeApiError),
            "claude_auth_error" => Ok(Self::ClaudeAuthError),
            "claude_billing_error" => Ok(Self::ClaudeBillingError),
            "claude_rate_limit" => Ok(Self::ClaudeRateLimit),
            "claude_model_unavailable" => Ok(Self::ClaudeModelUnavailable),
            "claude_setup_required" => Ok(Self::ClaudeSetupRequired),
            "runner_timeout" => Ok(Self::RunnerTimeout),
            "client_disconnected" => Ok(Self::ClientDisconnected),
            _ => Err(format!("unknown failure category: {s}")),
        }
    }
}

impl Serialize for FailureCategory {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for FailureCategory {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse::<Self>().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Isolation {
    None,
    Worktree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchProfile {
    Bridge,
    Bare,
    Unblocked,
}

impl LaunchProfile {
    pub const ALL: [Self; 3] = [Self::Bridge, Self::Bare, Self::Unblocked];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bridge => "bridge",
            Self::Bare => "bare",
            Self::Unblocked => "unblocked",
        }
    }
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

pub fn launch_profiles() -> Vec<&'static str> {
    LaunchProfile::ALL
        .iter()
        .map(|profile| profile.as_str())
        .collect()
}

fn join_provider_names() -> String {
    provider_names().join(", ")
}

fn join_task_modes() -> String {
    task_modes().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_category_round_trip_serialization() {
        let variants = [
            FailureCategory::ProviderTimeout,
            FailureCategory::ProviderOutputError,
            FailureCategory::ProviderExitError,
            FailureCategory::ProviderStartError,
            FailureCategory::ProviderSandboxDenied,
            FailureCategory::HostRunnerUnavailable,
            FailureCategory::WorktreeCleanupFailed,
            FailureCategory::WorktreeReclaimFailed,
            FailureCategory::AgentDirCleanupFailed,
            FailureCategory::TranscriptUnavailable,
            FailureCategory::ClaudeApiError,
            FailureCategory::ClaudeAuthError,
            FailureCategory::ClaudeBillingError,
            FailureCategory::ClaudeRateLimit,
            FailureCategory::ClaudeModelUnavailable,
            FailureCategory::ClaudeSetupRequired,
            FailureCategory::RunnerTimeout,
            FailureCategory::ClientDisconnected,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: FailureCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed, "round-trip failed for {variant:?}");
            // Verify the serialized form is a string.
            let value = serde_json::to_value(variant).unwrap();
            assert!(value.is_string(), "expected string for {variant:?}");
        }
    }

    #[test]
    fn failure_category_from_str_round_trip() {
        for expected in [
            FailureCategory::ProviderTimeout,
            FailureCategory::ClaudeApiError,
            FailureCategory::ClientDisconnected,
        ] {
            let s = expected.as_str();
            let parsed: FailureCategory = s.parse().unwrap();
            assert_eq!(expected, parsed);
        }
    }

    #[test]
    fn failure_category_transient_retry_set_matches_spec() {
        let cases = [
            (FailureCategory::ProviderTimeout, true),
            (FailureCategory::ProviderOutputError, false),
            (FailureCategory::ProviderExitError, false),
            (FailureCategory::ProviderStartError, true),
            (FailureCategory::ProviderSandboxDenied, false),
            (FailureCategory::HostRunnerUnavailable, true),
            (FailureCategory::WorktreeCleanupFailed, false),
            (FailureCategory::WorktreeReclaimFailed, false),
            (FailureCategory::AgentDirCleanupFailed, false),
            (FailureCategory::TranscriptUnavailable, false),
            (FailureCategory::ClaudeApiError, false),
            (FailureCategory::ClaudeAuthError, false),
            (FailureCategory::ClaudeBillingError, false),
            (FailureCategory::ClaudeRateLimit, false),
            (FailureCategory::ClaudeModelUnavailable, false),
            (FailureCategory::ClaudeSetupRequired, false),
            (FailureCategory::RunnerTimeout, false),
            (FailureCategory::ClientDisconnected, false),
        ];
        for (category, retryable) in cases {
            assert_eq!(
                category.is_transient(),
                retryable,
                "unexpected retry classification for {category:?}"
            );
        }
    }

    #[test]
    fn failure_category_rejects_unknown_strings() {
        let result = "totally_bogus_category".parse::<FailureCategory>();
        assert!(result.is_err());
    }
}
