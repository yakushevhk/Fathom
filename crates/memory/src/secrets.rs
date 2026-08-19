//! Secret detection for memory writes (Memora STORAGE pattern).
//!
//! Research agents routinely see API keys and tokens in fetched pages,
//! config files and shell output. Anything absorbed into long-term memory
//! would persist and later leak into prompts, exports and digests — so
//! facts containing secrets are rejected at write time.

use regex::Regex;
use std::sync::OnceLock;

/// One detected secret class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    OpenAiKey,
    AnthropicKey,
    AwsAccessKey,
    GitHubToken,
    SlackToken,
    BearerToken,
    PrivateKey,
    GenericApiKey,
}

impl SecretKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::OpenAiKey => "OpenAI-style API key (sk-...)",
            Self::AnthropicKey => "Anthropic API key (sk-ant-...)",
            Self::AwsAccessKey => "AWS access key (AKIA...)",
            Self::GitHubToken => "GitHub token",
            Self::SlackToken => "Slack token",
            Self::BearerToken => "Bearer authorization token",
            Self::PrivateKey => "PEM private key block",
            Self::GenericApiKey => "generic api_key/secret assignment",
        }
    }
}

struct Patterns {
    openai: Regex,
    anthropic: Regex,
    aws: Regex,
    github: Regex,
    slack: Regex,
    bearer: Regex,
    private_key: Regex,
    generic: Regex,
}

fn patterns() -> &'static Patterns {
    static PATTERNS: OnceLock<Patterns> = OnceLock::new();
    PATTERNS.get_or_init(|| Patterns {
        // sk-... (OpenAI, OpenRouter sk-or-, DeepSeek, many gateways).
        openai: Regex::new(r"sk-[A-Za-z0-9_-]{20,}").unwrap(),
        anthropic: Regex::new(r"sk-ant-[A-Za-z0-9_-]{20,}").unwrap(),
        aws: Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
        github: Regex::new(r"(ghp_[A-Za-z0-9]{36}|github_pat_[A-Za-z0-9_]{20,})").unwrap(),
        slack: Regex::new(r"xox[baprs]-[A-Za-z0-9-]{10,}").unwrap(),
        bearer: Regex::new(r"(?i)bearer\s+[A-Za-z0-9._\-]{24,}").unwrap(),
        private_key: Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----").unwrap(),
        // `api_key = "..."` / `"apiKey": "..."` / `SECRET=...` assignments
        // with a long enough value. An optional quote may sit between the
        // key name and the colon (`"apiKey": ...`).
        generic: Regex::new(
            r#"(?i)(api[_-]?key|secret|token|passwd|password)['"]?\s*[:=]\s*['"]?[A-Za-z0-9_/+\-]{16,}"#,
        )
        .unwrap(),
    })
}

/// Scan text for known secret patterns. Returns the distinct classes found
/// (empty = clean).
pub fn detect_secrets(text: &str) -> Vec<SecretKind> {
    let p = patterns();
    let mut found = Vec::new();
    let mut push = |k: SecretKind| {
        if !found.contains(&k) {
            found.push(k);
        }
    };
    if p.anthropic.is_match(text) {
        push(SecretKind::AnthropicKey);
    }
    if p.openai.is_match(text) {
        // sk-ant- also matches sk-; only report OpenAI-style when the
        // Anthropic variant did not already explain the match.
        let without_anthropic = p.anthropic.replace_all(text, "");
        if p.openai.is_match(&without_anthropic) {
            push(SecretKind::OpenAiKey);
        }
    }
    if p.aws.is_match(text) {
        push(SecretKind::AwsAccessKey);
    }
    if p.github.is_match(text) {
        push(SecretKind::GitHubToken);
    }
    if p.slack.is_match(text) {
        push(SecretKind::SlackToken);
    }
    if p.bearer.is_match(text) {
        push(SecretKind::BearerToken);
    }
    if p.private_key.is_match(text) {
        push(SecretKind::PrivateKey);
    }
    if p.generic.is_match(text) {
        push(SecretKind::GenericApiKey);
    }
    found
}

/// Human-readable rejection reason for absorb reports.
pub fn rejection_reason(kinds: &[SecretKind]) -> String {
    let labels: Vec<&str> = kinds.iter().map(|k| k.label()).collect();
    format!(
        "memory write rejected: possible secret material ({})",
        labels.join(", ")
    )
}

/// Replace known secret substrings with a redaction placeholder. Unlike
/// rejection (which drops the whole record), this lets a tool surface the
/// *surrounding* text while guaranteeing no key material ever reaches logs,
/// digests or exports (read-side safe display).
pub fn redact_secrets(text: &str) -> String {
    let p = patterns();
    let mut out = text.to_string();
    for re in [
        &p.private_key, &p.anthropic, &p.openai, &p.aws, &p.github, &p.slack,
        // bearer + generic assignments: keep the key name, mask the value.
        &p.generic, &p.bearer,
    ] {
        out = re.replace_all(&out, "[REDACTED]").into_owned();
    }
    out
}

/// Whether `value` already looks like a masked/wire placeholder (so a reader
/// can tell a real-but-masked secret from a literal value of that shape).
pub fn looks_masked(value: &str) -> bool {
    let v = value.trim();
    v.starts_with("[REDACTED]") || v.starts_with("***") || (v.starts_with("sk-") && v.len() < 24)
}

/// Short wire-safe projection of a secret: keep the first 8 visible chars,
/// mask the rest, so settings screens can show *something* without leaking.
pub fn mask_preview(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    let visible: String = value.chars().take(8).collect();
    if visible.is_empty() {
        "***".to_string()
    } else {
        format!("{visible}...")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_text_passes() {
        assert!(detect_secrets("Acme LLC was founded in 2019, CEO is Alice").is_empty());
        assert!(detect_secrets("contact: info@acme.com, +7 999 000-11-22").is_empty());
        // Short assignment values are not flagged.
        assert!(detect_secrets("api_key = \"short\"").is_empty());
    }

    #[test]
    fn openai_key_detected() {
        let kinds = detect_secrets("key is sk-proj-abcdefghijklmnopqrstuvwx123");
        assert!(kinds.contains(&SecretKind::OpenAiKey));
    }

    #[test]
    fn redact_removes_keys_but_keeps_context() {
        let input = "endpoint https://api.example.com, credentials sk-proj-abcdefghijklmnopqrstuvwx, done";
        let out = redact_secrets(input);
        assert!(!out.contains("sk-proj"), "key must be redacted: {out}");
        assert!(out.contains("https://api.example.com"));
        assert!(out.contains("[REDACTED]"));
    }

    #[test]
    fn mask_and_detection_helpers() {
        assert!(looks_masked("[REDACTED]"));
        assert!(looks_masked("***"));
        assert!(!looks_masked("api.example.com"));
        assert_eq!(mask_preview("sk-abc123456789"), "sk-abc12...");
    }

    #[test]
    fn anthropic_key_detected_and_not_double_counted_as_openai() {
        let kinds = detect_secrets("sk-ant-api03-abcdefghijklmnopqrstuvwxyz");
        assert!(kinds.contains(&SecretKind::AnthropicKey));
        assert!(!kinds.contains(&SecretKind::OpenAiKey));
    }

    #[test]
    fn aws_github_slack_detected() {
        assert!(detect_secrets("AKIAIOSFODNN7EXAMPLE").contains(&SecretKind::AwsAccessKey));
        assert!(detect_secrets("ghp_abcdefghijklmnopqrstuvwxyz0123456789")
            .contains(&SecretKind::GitHubToken));
        assert!(detect_secrets("xoxb-1234567890-abcdef")
            .contains(&SecretKind::SlackToken));
    }

    #[test]
    fn bearer_and_private_key_detected() {
        assert!(detect_secrets("Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.payload.sig")
            .contains(&SecretKind::BearerToken));
        assert!(detect_secrets("-----BEGIN RSA PRIVATE KEY-----\nMIIE...")
            .contains(&SecretKind::PrivateKey));
    }

    #[test]
    fn generic_assignment_detected() {
        assert!(detect_secrets("smtp_password = \"verysecretpassphrase42\"")
            .contains(&SecretKind::GenericApiKey));
        assert!(detect_secrets(r#"{"apiKey": "abcdef1234567890abcdef"}"#)
            .contains(&SecretKind::GenericApiKey));
    }

    #[test]
    fn rejection_reason_lists_labels() {
        let reason = rejection_reason(&[SecretKind::OpenAiKey]);
        assert!(reason.contains("OpenAI"));
        assert!(reason.contains("rejected"));
    }
}
