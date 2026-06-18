use agent_bridge_mcp::domain::{FailureCategory, Isolation, LaunchProfile, ProviderKind, TaskMode};
use agent_bridge_mcp::router::{
    AttemptDisposition, AttemptEvidence, RoutedAttemptInput, RouterPolicy, RouterStopReason,
    classify_attempt,
};
use serde_json::json;

#[test]
fn router_policy_accepts_only_codex_and_claude() {
    assert!(RouterPolicy::new(vec![ProviderKind::Codex, ProviderKind::Claude]).is_ok());

    let error = RouterPolicy::new(vec![ProviderKind::Cursor]).unwrap_err();

    assert_eq!(error.to_string(), "router provider is unsupported: cursor");
}

#[test]
fn final_text_is_trusted_finality() {
    let evidence = AttemptEvidence {
        final_text_present: true,
        failure_category: Some(FailureCategory::ProviderStartError),
        stop_reason: None,
    };

    assert_eq!(
        classify_attempt(&evidence),
        AttemptDisposition::TrustedFinal
    );
}

#[test]
fn semantic_stop_reason_overrides_final_text() {
    let evidence = AttemptEvidence {
        final_text_present: true,
        failure_category: Some(FailureCategory::ProviderOutputError),
        stop_reason: Some(RouterStopReason::Refusal),
    };

    assert_eq!(classify_attempt(&evidence), AttemptDisposition::Blocker);
}

#[test]
fn launch_failure_before_finality_can_fail_over() {
    let evidence = AttemptEvidence {
        final_text_present: false,
        failure_category: Some(FailureCategory::ProviderStartError),
        stop_reason: None,
    };

    assert_eq!(
        classify_attempt(&evidence),
        AttemptDisposition::FailoverEligible
    );
}

#[test]
fn semantic_blockers_do_not_fail_over() {
    for evidence in [
        AttemptEvidence {
            final_text_present: false,
            failure_category: None,
            stop_reason: Some(RouterStopReason::Refusal),
        },
        AttemptEvidence {
            final_text_present: false,
            failure_category: None,
            stop_reason: Some(RouterStopReason::Cancelled),
        },
        AttemptEvidence {
            final_text_present: false,
            failure_category: Some(FailureCategory::ClaudeAuthError),
            stop_reason: None,
        },
        AttemptEvidence {
            final_text_present: false,
            failure_category: Some(FailureCategory::ClaudeBillingError),
            stop_reason: None,
        },
    ] {
        assert_eq!(classify_attempt(&evidence), AttemptDisposition::Blocker);
    }
}

#[test]
fn routed_attempt_preserves_spawn_workspace_arguments() {
    let input = RoutedAttemptInput {
        provider: ProviderKind::Codex,
        mode: TaskMode::Implement,
        prompt: "make the change".to_string(),
        title: Some("router attempt".to_string()),
        cwd: Some("/repo".to_string()),
        timeout_seconds: Some(30),
        isolation: Some(Isolation::Worktree),
        worktree_name: Some("router-attempt".to_string()),
        profile: Some(LaunchProfile::Unblocked),
    };

    assert_eq!(
        input.spawn_arguments(),
        json!({
            "provider": "codex",
            "mode": "implement",
            "prompt": "make the change",
            "title": "router attempt",
            "cwd": "/repo",
            "timeoutSeconds": 30,
            "isolation": "worktree",
            "worktreeName": "router-attempt",
            "profile": "unblocked"
        })
    );
}
