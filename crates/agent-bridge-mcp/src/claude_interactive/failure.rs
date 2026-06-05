use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeFailureCategory {
    Auth,
    Billing,
    RateLimit,
    ModelUnavailable,
    Api,
}

impl ClaudeFailureCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auth => "claude_auth_error",
            Self::Billing => "claude_billing_error",
            Self::RateLimit => "claude_rate_limit",
            Self::ModelUnavailable => "claude_model_unavailable",
            Self::Api => "claude_api_error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopFailure {
    pub error: String,
    pub error_details: Option<String>,
    pub last_assistant_message: Option<String>,
    pub category: ClaudeFailureCategory,
}

pub fn parse_stop_failure(payload: &Value) -> Option<StopFailure> {
    if payload.get("hook_event_name").and_then(Value::as_str) != Some("StopFailure") {
        return None;
    }
    let error = payload
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    Some(StopFailure {
        category: category_for_error(&error),
        error,
        error_details: payload
            .get("error_details")
            .and_then(Value::as_str)
            .map(str::to_string),
        last_assistant_message: payload
            .get("last_assistant_message")
            .and_then(Value::as_str)
            .filter(|message| !message.is_empty())
            .map(str::to_string),
    })
}

fn category_for_error(error: &str) -> ClaudeFailureCategory {
    match error {
        "authentication_failed" | "oauth_org_not_allowed" => ClaudeFailureCategory::Auth,
        "billing_error" => ClaudeFailureCategory::Billing,
        "rate_limit" => ClaudeFailureCategory::RateLimit,
        "model_not_found" => ClaudeFailureCategory::ModelUnavailable,
        "invalid_request" | "server_error" | "max_output_tokens" | "unknown" => {
            ClaudeFailureCategory::Api
        }
        _ => ClaudeFailureCategory::Api,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::path::Path;

    const FIXTURE_DIR: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/interactive_claude"
    );

    #[test]
    fn stop_failure_maps_canonical_claude_errors() {
        for (fixture, category) in [
            ("stop_failure_auth.json", ClaudeFailureCategory::Auth),
            ("stop_failure_billing.json", ClaudeFailureCategory::Billing),
            (
                "stop_failure_rate_limit.json",
                ClaudeFailureCategory::RateLimit,
            ),
            ("stop_failure_unknown.json", ClaudeFailureCategory::Api),
        ] {
            let failure = parse_stop_failure(&fixture_payload(fixture)).unwrap();
            assert_eq!(failure.category, category);
            assert_eq!(failure.category.as_str(), category.as_str());
            assert!(failure.last_assistant_message.is_some());
        }
    }

    #[test]
    fn stop_failure_maps_model_and_oauth_errors() {
        for (error, category) in [
            ("oauth_org_not_allowed", ClaudeFailureCategory::Auth),
            ("model_not_found", ClaudeFailureCategory::ModelUnavailable),
            ("server_error", ClaudeFailureCategory::Api),
            ("invalid_request", ClaudeFailureCategory::Api),
            ("max_output_tokens", ClaudeFailureCategory::Api),
            ("future_error", ClaudeFailureCategory::Api),
        ] {
            let payload = serde_json::json!({
                "hook_event_name": "StopFailure",
                "error": error,
                "error_details": "details",
                "last_assistant_message": "partial"
            });
            let failure = parse_stop_failure(&payload).unwrap();
            assert_eq!(failure.category, category, "{error}");
            assert_eq!(failure.error_details.as_deref(), Some("details"));
            assert_eq!(failure.last_assistant_message.as_deref(), Some("partial"));
        }
    }

    #[test]
    fn non_stop_failure_payload_is_ignored() {
        let payload = serde_json::json!({"hook_event_name": "Stop"});
        assert!(parse_stop_failure(&payload).is_none());
    }

    fn fixture_payload(name: &str) -> Value {
        let path = Path::new(FIXTURE_DIR).join("hooks").join(name);
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }
}
