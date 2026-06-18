use crate::domain::{FailureCategory, Isolation, LaunchProfile, ProviderKind, TaskMode};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutedTurnInput {
    pub prompt: String,
    pub policy: RouterPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedAttemptInput {
    pub provider: ProviderKind,
    pub mode: TaskMode,
    pub prompt: String,
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub timeout_seconds: Option<i64>,
    pub isolation: Option<Isolation>,
    pub worktree_name: Option<String>,
    pub profile: Option<LaunchProfile>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoutedAttemptExecution {
    pub agent_id: String,
    pub evidence_ref: RoutedAttemptEvidenceRef,
    pub wait_status: Value,
    pub result: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedAttemptEvidenceRef {
    pub agent_id: String,
    pub result_sections: Vec<&'static str>,
    pub transcript_available: bool,
}

impl RoutedAttemptInput {
    pub fn spawn_arguments(&self) -> Value {
        let mut arguments = Map::new();
        arguments.insert("provider".to_string(), json!(self.provider));
        arguments.insert("mode".to_string(), json!(self.mode));
        arguments.insert("prompt".to_string(), json!(self.prompt));
        insert_optional(&mut arguments, "title", self.title.as_deref());
        insert_optional(&mut arguments, "cwd", self.cwd.as_deref());
        insert_optional(&mut arguments, "timeoutSeconds", self.timeout_seconds);
        insert_optional(&mut arguments, "isolation", self.isolation);
        insert_optional(
            &mut arguments,
            "worktreeName",
            self.worktree_name.as_deref(),
        );
        insert_optional(&mut arguments, "profile", self.profile);
        Value::Object(arguments)
    }
}

impl RoutedAttemptEvidenceRef {
    pub fn from_result(agent_id: String, result: &Value) -> Self {
        let transcript_available = result
            .get("reviewPacket")
            .and_then(|review_packet| review_packet.get("transcriptAvailable"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Self {
            agent_id,
            result_sections: vec!["summary", "changedFiles"],
            transcript_available,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouterPolicy {
    pub candidates: Vec<ProviderKind>,
}

impl RouterPolicy {
    pub fn new(candidates: Vec<ProviderKind>) -> Result<Self, RouterPolicyError> {
        if let Some(provider) = candidates
            .iter()
            .copied()
            .find(|provider| !router_provider(*provider))
        {
            return Err(RouterPolicyError::UnsupportedProvider(provider));
        }
        Ok(Self { candidates })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouterPolicyError {
    UnsupportedProvider(ProviderKind),
}

impl fmt::Display for RouterPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProvider(provider) => {
                write!(f, "router provider is unsupported: {}", provider.as_str())
            }
        }
    }
}

impl std::error::Error for RouterPolicyError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptDisposition {
    TrustedFinal,
    FailoverEligible,
    Blocker,
    TerminalFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptEvidence {
    pub final_text_present: bool,
    pub failure_category: Option<FailureCategory>,
    pub stop_reason: Option<RouterStopReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouterStopReason {
    EndTurn,
    Refusal,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptOutcome {
    pub provider: ProviderKind,
    pub disposition: AttemptDisposition,
    pub failure_category: Option<FailureCategory>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutedTurnResult {
    pub provider: ProviderKind,
    pub terminal_kind: RoutedTerminalKind,
    pub final_text: Option<String>,
    pub failure_category: Option<FailureCategory>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutedTerminalKind {
    Answer,
    Blocker,
    Failure,
}

pub fn classify_attempt(evidence: &AttemptEvidence) -> AttemptDisposition {
    if matches!(
        evidence.stop_reason,
        Some(RouterStopReason::Refusal | RouterStopReason::Cancelled)
    ) || evidence.failure_category.is_some_and(blocker_failure)
    {
        return AttemptDisposition::Blocker;
    }
    if evidence.final_text_present {
        return AttemptDisposition::TrustedFinal;
    }
    if evidence
        .failure_category
        .is_some_and(failover_eligible_failure)
    {
        return AttemptDisposition::FailoverEligible;
    }
    AttemptDisposition::TerminalFailure
}

fn router_provider(provider: ProviderKind) -> bool {
    matches!(provider, ProviderKind::Codex | ProviderKind::Claude)
}

fn blocker_failure(category: FailureCategory) -> bool {
    matches!(
        category,
        FailureCategory::ClaudeAuthError
            | FailureCategory::ClaudeBillingError
            | FailureCategory::ClaudeSetupRequired
    )
}

fn failover_eligible_failure(category: FailureCategory) -> bool {
    matches!(
        category,
        FailureCategory::ProviderStartError
            | FailureCategory::ProviderTimeout
            | FailureCategory::ProviderExitError
            | FailureCategory::HostRunnerUnavailable
            | FailureCategory::RunnerTimeout
            | FailureCategory::ClientDisconnected
    )
}

fn insert_optional<T: Serialize>(arguments: &mut Map<String, Value>, key: &str, value: Option<T>) {
    if let Some(value) = value {
        arguments.insert(key.to_string(), json!(value));
    }
}
