//! The runtime removes configured secrets before emitting model text. This
//! browser boundary additionally removes recognizable credentials and terminal
//! controls, without applying the lossy 512-character action-summary format.

use std::sync::OnceLock;

use regex::{Captures, Regex};

struct Patterns {
    controls: Regex,
    private_key: Regex,
    authorization: Regex,
    assignment: Regex,
    provider_token: Regex,
    url_credentials: Regex,
}

fn patterns() -> &'static Patterns {
    static PATTERNS: OnceLock<Patterns> = OnceLock::new();
    PATTERNS.get_or_init(|| Patterns {
        controls: Regex::new(concat!(
            r"\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07\x1b]*(?:\x07|\x1b\\))",
            r"|\p{Cf}|[\x00-\x08\x0b\x0c\x0e-\x1f\x7f-\x9f]",
        )).expect("static control pattern"),
        private_key: Regex::new(r"(?s)-----BEGIN [^-\r\n]*PRIVATE KEY-----.*?(?:-----END [^-\r\n]*PRIVATE KEY-----|$)").expect("static private key pattern"),
        authorization: Regex::new(r#"(?i)(\b(?:authorization\s*:\s*(?:bearer|basic)|bearer)\s+)[^\s"'`,;<>]+"#).expect("static authorization pattern"),
        assignment: Regex::new(r#"(?i)(\b(?:api[_-]?key|access[_-]?token|auth[_-]?token|password|passwd|client[_-]?secret|secret|token)\b["']?\s*[:=]\s*["']?)[^\s"'`,;&<>)]+"#).expect("static credential assignment pattern"),
        provider_token: Regex::new(r"(?i)\b(?:(?:sk[-_]|ghp_|github_pat_|xox[baprs]-)[A-Za-z0-9._-]+|AKIA[A-Z0-9]{12,})\b").expect("static provider token pattern"),
        url_credentials: Regex::new(r"(?i)(https?://)[^/\s:@]+:[^/\s@]+@").expect("static URL credential pattern"),
    })
}

pub(super) fn redact_message(input: &str) -> String {
    let patterns = patterns();
    // Remove invisible separators before detection, so they cannot hide a token.
    // CR, LF and tab are meaningful in Markdown and source code.
    let mut text = patterns.controls.replace_all(input, "").into_owned();
    text = patterns
        .private_key
        .replace_all(&text, "[REDACTED_PRIVATE_KEY]")
        .into_owned();
    for pattern in [&patterns.authorization, &patterns.assignment] {
        text = pattern
            .replace_all(&text, |capture: &Captures<'_>| {
                format!("{}[REDACTED]", &capture[1])
            })
            .into_owned();
    }
    text = patterns
        .provider_token
        .replace_all(&text, "[REDACTED]")
        .into_owned();
    patterns
        .url_credentials
        .replace_all(&text, "${1}[REDACTED]@")
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_and_controls_are_removed_without_rewriting_prose() {
        let fixtures = [
            ("Authorization: Bearer credential-value", "Authorization: Bearer [REDACTED]"),
            ("api_key=credential-value", "api_key=[REDACTED]"),
            ("{\"password\":\"credential-value\"}", "{\"password\":\"[REDACTED]\"}"),
            ("`sk-live-private-value`", "`[REDACTED]`"),
            ("ghp_private-value", "[REDACTED]"),
            ("https://alice:credential-value@example.org/doc", "https://[REDACTED]@example.org/doc"),
            ("[docs](https://example.org/doc?api_key=credential-value)", "[docs](https://example.org/doc?api_key=[REDACTED])"),
            ("sk-\0live-private-value", "[REDACTED]"),
            ("sk-\u{202e}live-private-value", "[REDACTED]"),
            ("\u{001b}[31mheading\u{001b}[0m\r\n\tcode", "heading\r\n\tcode"),
            ("before\n-----BEGIN PRIVATE KEY-----\nprivate-data\n-----END PRIVATE KEY-----\nafter", "before\n[REDACTED_PRIVATE_KEY]\nafter"),
        ];
        for (input, expected) in fixtures {
            let safe = redact_message(input);
            assert_eq!(safe, expected);
            assert_eq!(redact_message(&safe), safe, "redaction must be idempotent");
        }
        let prose = "Basic sorting. The token_count is 42. [docs](https://example.org/doc?q=sort)\n\tstd::cout << \"hello\\n\";";
        assert_eq!(redact_message(prose), prose);
    }
}
