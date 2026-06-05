use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURE_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/interactive_claude"
);

#[test]
fn interactive_claude_fixtures_cover_required_scenarios() {
    let fixture_dir = Path::new(FIXTURE_DIR);
    let expected = [
        "fake_interactive_claude.sh",
        "terminal_probes.txt",
        "hooks/session_start.json",
        "hooks/stop.json",
        "hooks/stop_failure_auth.json",
        "hooks/stop_failure_billing.json",
        "hooks/stop_failure_rate_limit.json",
        "hooks/stop_failure_unknown.json",
        "setup_prompts/login.txt",
        "setup_prompts/workspace_trust.txt",
        "transcripts/success.jsonl",
        "transcripts/malformed.jsonl",
        "transcripts/no_assistant.jsonl",
    ];

    for relative in expected {
        assert!(
            fixture_dir.join(relative).is_file(),
            "missing fixture {relative}"
        );
    }

    let script = fixture_dir.join("fake_interactive_claude.sh");
    let mode = std::fs::metadata(&script).unwrap().permissions().mode();
    assert_ne!(mode & 0o111, 0, "fake Claude fixture must be executable");

    for relative in [
        "hooks/session_start.json",
        "hooks/stop.json",
        "hooks/stop_failure_auth.json",
        "hooks/stop_failure_billing.json",
        "hooks/stop_failure_rate_limit.json",
        "hooks/stop_failure_unknown.json",
    ] {
        let payload = read_json(&fixture_dir.join(relative));
        assert!(payload["hook_event_name"].is_string(), "{relative}");
    }

    for error in [
        ("hooks/stop_failure_auth.json", "authentication_failed"),
        ("hooks/stop_failure_billing.json", "billing_error"),
        ("hooks/stop_failure_rate_limit.json", "rate_limit"),
        ("hooks/stop_failure_unknown.json", "future_new_error"),
    ] {
        let payload = read_json(&fixture_dir.join(error.0));
        assert_eq!(payload["hook_event_name"], "StopFailure");
        assert_eq!(payload["error"], error.1);
        assert!(payload["last_assistant_message"].is_string());
    }

    assert_jsonl(&fixture_dir.join("transcripts/success.jsonl"), true);
    assert_jsonl(&fixture_dir.join("transcripts/no_assistant.jsonl"), true);
    assert_jsonl(&fixture_dir.join("transcripts/malformed.jsonl"), false);

    let login = std::fs::read_to_string(fixture_dir.join("setup_prompts/login.txt")).unwrap();
    assert!(login.contains("/login"));
    let trust =
        std::fs::read_to_string(fixture_dir.join("setup_prompts/workspace_trust.txt")).unwrap();
    assert!(trust.to_lowercase().contains("trust"));
    assert!(trust.to_lowercase().contains("folder"));
}

#[test]
fn fake_interactive_claude_emits_probe_bytes_and_stopfailure_payloads() {
    let fixture_dir = Path::new(FIXTURE_DIR);
    let script = fixture_dir.join("fake_interactive_claude.sh");

    let probes = Command::new(&script)
        .env("FAKE_CLAUDE_SCENARIO", "terminal-probes")
        .output()
        .unwrap();
    assert!(probes.status.success());
    assert!(probes.stdout.windows(3).any(|window| window == b"\x1b[c"));
    assert!(probes.stdout.windows(4).any(|window| window == b"\x1b[>c"));
    assert!(probes.stdout.windows(4).any(|window| window == b"\x1b[6n"));
    assert!(probes.stdout.windows(4).any(|window| window == b"\x1b[>q"));
    assert!(probes.stdout.windows(5).any(|window| window == b"\x1b[18t"));

    let stop_failure = Command::new(&script)
        .env("FAKE_CLAUDE_SCENARIO", "stop-failure-rate-limit")
        .output()
        .unwrap();
    assert!(stop_failure.status.success());
    let payload: Value = serde_json::from_slice(&stop_failure.stdout).unwrap();
    assert_eq!(payload["hook_event_name"], "StopFailure");
    assert_eq!(payload["error"], "rate_limit");
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn assert_jsonl(path: &PathBuf, all_valid: bool) {
    let text = std::fs::read_to_string(path).unwrap();
    let mut saw_invalid = false;
    for line in text.lines() {
        if serde_json::from_str::<Value>(line).is_err() {
            saw_invalid = true;
        }
    }
    assert_eq!(
        !saw_invalid, all_valid,
        "unexpected JSONL validity: {path:?}"
    );
}
