use crate::domain::{FailureCategory, ProviderKind};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutedTurnInput {
    pub prompt: String,
    pub policy: RouterPolicy,
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
    if evidence.final_text_present {
        return AttemptDisposition::TrustedFinal;
    }
    if matches!(
        evidence.stop_reason,
        Some(RouterStopReason::Refusal | RouterStopReason::Cancelled)
    ) || evidence.failure_category.is_some_and(blocker_failure)
    {
        return AttemptDisposition::Blocker;
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
