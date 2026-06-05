use serde_json::Value;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptSource {
    Transcript,
    StopLastAssistantMessage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopResult {
    pub final_text: String,
    pub source: TranscriptSource,
    pub session_id: Option<String>,
    pub transcript_path: Option<PathBuf>,
    pub fallback_used: bool,
}

pub async fn resolve_stop_result(
    stop_payload: &Value,
    retry_budget: Duration,
) -> io::Result<StopResult> {
    let session_id = stop_payload
        .get("session_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let last_assistant_message = stop_payload
        .get("last_assistant_message")
        .and_then(Value::as_str)
        .filter(|message| !message.is_empty())
        .map(str::to_string);
    if let Some(path) = stop_payload.get("transcript_path").and_then(Value::as_str) {
        match validate_transcript_path(Path::new(path)) {
            Ok(transcript_path) => {
                if let Ok(Some(final_text)) =
                    read_transcript_with_retry(&transcript_path, retry_budget).await
                {
                    return Ok(StopResult {
                        final_text,
                        source: TranscriptSource::Transcript,
                        session_id,
                        transcript_path: Some(transcript_path),
                        fallback_used: false,
                    });
                }
                if let Some(final_text) = last_assistant_message {
                    return Ok(StopResult {
                        final_text,
                        source: TranscriptSource::StopLastAssistantMessage,
                        session_id,
                        transcript_path: Some(transcript_path),
                        fallback_used: true,
                    });
                }
            }
            Err(_) => {
                if let Some(final_text) = last_assistant_message {
                    return Ok(StopResult {
                        final_text,
                        source: TranscriptSource::StopLastAssistantMessage,
                        session_id,
                        transcript_path: None,
                        fallback_used: true,
                    });
                }
            }
        }
    } else if let Some(final_text) = last_assistant_message {
        return Ok(StopResult {
            final_text,
            source: TranscriptSource::StopLastAssistantMessage,
            session_id,
            transcript_path: None,
            fallback_used: true,
        });
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "Stop payload did not contain usable transcript or last assistant message",
    ))
}

pub fn validate_transcript_path(path: &Path) -> io::Result<PathBuf> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "transcript path must be absolute",
        ));
    }
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "transcript path must not be a symlink",
        ));
    }
    let canonical = fs::canonicalize(path)?;
    let metadata = fs::metadata(&canonical)?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "transcript path must be a regular file",
        ));
    }
    Ok(canonical)
}

async fn read_transcript_with_retry(
    path: &Path,
    retry_budget: Duration,
) -> io::Result<Option<String>> {
    let started = std::time::Instant::now();
    loop {
        match parse_transcript(path) {
            Ok(Some(final_text)) => return Ok(Some(final_text)),
            Ok(None) if started.elapsed() < retry_budget => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Ok(None) => return Ok(None),
            Err(error) => return Err(error),
        }
    }
}

pub fn parse_transcript(path: &Path) -> io::Result<Option<String>> {
    let text = fs::read_to_string(path)?;
    let mut final_assistant_text = None;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let value: Value = serde_json::from_str(line).map_err(io::Error::other)?;
        if value.get("type").and_then(Value::as_str) == Some("assistant")
            && let Some(text) = assistant_text(&value)
        {
            final_assistant_text = Some(text);
        }
    }
    Ok(final_assistant_text)
}

fn assistant_text(value: &Value) -> Option<String> {
    let content = value
        .get("message")?
        .get("content")?
        .as_array()?
        .iter()
        .filter_map(|part| {
            if part.get("type").and_then(Value::as_str) == Some("text") {
                part.get("text").and_then(Value::as_str)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if content.is_empty() {
        None
    } else {
        Some(content.join(""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::os::unix::fs::symlink;
    use std::time::{SystemTime, UNIX_EPOCH};

    const FIXTURE_DIR: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/interactive_claude"
    );

    #[tokio::test]
    async fn stop_result_prefers_transcript_assistant_text() {
        let transcript = copy_fixture("success.jsonl");
        let stop = json!({
            "session_id": "fake-session",
            "transcript_path": transcript,
            "last_assistant_message": "fallback text"
        });
        let result = resolve_stop_result(&stop, Duration::from_millis(0))
            .await
            .unwrap();
        assert_eq!(result.final_text, "fixture final response");
        assert_eq!(result.source, TranscriptSource::Transcript);
        assert!(!result.fallback_used);
    }

    #[tokio::test]
    async fn stop_result_falls_back_to_last_assistant_message() {
        let transcript = copy_fixture("no_assistant.jsonl");
        let stop = json!({
            "session_id": "fake-session",
            "transcript_path": transcript,
            "last_assistant_message": "fallback text"
        });
        let result = resolve_stop_result(&stop, Duration::from_millis(0))
            .await
            .unwrap();
        assert_eq!(result.final_text, "fallback text");
        assert_eq!(result.source, TranscriptSource::StopLastAssistantMessage);
        assert!(result.fallback_used);
    }

    #[tokio::test]
    async fn stop_result_rejects_unsafe_transcript_path_and_uses_fallback() {
        let target = copy_fixture("success.jsonl");
        let link = temp_path("transcript-link");
        symlink(target, &link).unwrap();
        let stop = json!({
            "session_id": "fake-session",
            "transcript_path": link,
            "last_assistant_message": "safe fallback"
        });
        let result = resolve_stop_result(&stop, Duration::from_millis(0))
            .await
            .unwrap();
        assert_eq!(result.final_text, "safe fallback");
        assert_eq!(result.transcript_path, None);
        assert!(result.fallback_used);
    }

    #[test]
    fn malformed_transcript_returns_parse_error() {
        let transcript = copy_fixture("malformed.jsonl");
        let error = parse_transcript(&transcript).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
    }

    fn copy_fixture(name: &str) -> PathBuf {
        let source = Path::new(FIXTURE_DIR).join("transcripts").join(name);
        let target = temp_path(name);
        fs::copy(source, &target).unwrap();
        target
    }

    fn temp_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "agent-bridge-transcript-{label}-{}-{nonce}",
            std::process::id()
        ))
    }
}
