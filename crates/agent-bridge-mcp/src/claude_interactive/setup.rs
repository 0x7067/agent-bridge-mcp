const SETUP_SIGNATURES: &[&str] = &[
    "please run /login",
    "session expired",
    "oauth refresh token is no longer valid",
    "not logged in",
    "claude auth login",
    "permitted organization",
    "does not have access to claude",
    "api key authentication is not sufficient",
    "organization has disabled claude subscription",
    "forceloginmethod",
];

pub fn detect_setup_prompt(excerpt: &[u8]) -> Option<&'static str> {
    let stripped = strip_ansi(excerpt);
    let lower = stripped.to_lowercase();
    if lower.contains("trust") && lower.contains("folder") {
        return Some("workspace_trust_required");
    }
    SETUP_SIGNATURES
        .iter()
        .find(|signature| lower.contains(**signature))
        .copied()
}

pub fn strip_ansi(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut stripped = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            match chars.peek().copied() {
                Some('[') => {
                    chars.next();
                    for next in chars.by_ref() {
                        if ('@'..='~').contains(&next) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    let mut previous = '\0';
                    for next in chars.by_ref() {
                        if next == '\u{7}' || (previous == '\u{1b}' && next == '\\') {
                            break;
                        }
                        previous = next;
                    }
                }
                _ => {}
            }
        } else {
            stripped.push(ch);
        }
    }
    stripped
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const FIXTURE_DIR: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/interactive_claude"
    );

    #[test]
    fn detects_login_and_workspace_trust_prompts() {
        let login = std::fs::read(fixture_path("login.txt")).unwrap();
        let trust = std::fs::read(fixture_path("workspace_trust.txt")).unwrap();
        assert_eq!(detect_setup_prompt(&login), Some("please run /login"));
        assert_eq!(
            detect_setup_prompt(&trust),
            Some("workspace_trust_required")
        );
    }

    #[test]
    fn strips_ansi_before_matching_setup_prompt() {
        let excerpt = b"\x1b[31mSession expired. Please run /login to sign in again.\x1b[0m";
        assert_eq!(detect_setup_prompt(excerpt), Some("please run /login"));
        assert_eq!(
            strip_ansi(excerpt),
            "Session expired. Please run /login to sign in again."
        );
    }

    #[test]
    fn ignores_normal_output() {
        assert_eq!(
            detect_setup_prompt(b"Claude is processing the prompt"),
            None
        );
    }

    fn fixture_path(name: &str) -> std::path::PathBuf {
        Path::new(FIXTURE_DIR).join("setup_prompts").join(name)
    }
}
