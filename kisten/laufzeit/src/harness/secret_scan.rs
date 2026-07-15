use std::collections::BTreeMap;
use std::fmt;
use std::sync::OnceLock;

use regex::Regex;
use secrecy::{ExposeSecret, SecretString};

const MAX_CONFIGURED_SECRET_BYTES: usize = 16 * 1024;
const MAX_CONFIGURED_SECRETS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretCategory {
    ConfiguredCredential,
    PrivateKey,
    AuthorizationHeader,
    ProviderToken,
    HighEntropyToken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SecretFinding {
    pub(crate) category: SecretCategory,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SecretScannerConfigError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TerminalObfuscation {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

pub(crate) struct SecretScanner {
    configured: Vec<SecretString>,
}

impl SecretScanner {
    pub(crate) fn try_new(configured: Vec<SecretString>) -> Result<Self, SecretScannerConfigError> {
        if configured.len() > MAX_CONFIGURED_SECRETS
            || configured
                .iter()
                .any(|secret| secret.expose_secret().len() > MAX_CONFIGURED_SECRET_BYTES)
        {
            return Err(SecretScannerConfigError);
        }
        Ok(Self { configured })
    }

    pub(crate) fn scan(&self, input: &str) -> Result<Option<SecretFinding>, TerminalObfuscation> {
        if let Some((category, start, end)) = self.detect(input) {
            return Ok(Some(SecretFinding {
                category,
                start,
                end,
            }));
        }
        let normalized = NormalizedText::without_terminal_controls(input);
        if let Some((category, start, end)) = self.detect(&normalized.text) {
            return Ok(Some(normalized.finding(category, start, end)));
        }
        if let Some((start, end)) = normalized.terminal_range {
            return Err(TerminalObfuscation { start, end });
        }
        Ok(None)
    }

    fn detect(&self, input: &str) -> Option<(SecretCategory, usize, usize)> {
        for secret in &self.configured {
            let secret = secret.expose_secret();
            if !secret.is_empty() {
                if let Some(start) = input.find(secret) {
                    return Some((
                        SecretCategory::ConfiguredCredential,
                        start,
                        start + secret.len(),
                    ));
                }
            }
        }
        for (pattern, category) in [
            (private_key_pattern(), SecretCategory::PrivateKey),
            (authorization_pattern(), SecretCategory::AuthorizationHeader),
            (provider_token_pattern(), SecretCategory::ProviderToken),
        ] {
            if let Some(matched) = pattern.find(input) {
                return Some((category, matched.start(), matched.end()));
            }
        }
        high_entropy_pattern()
            .find_iter(input)
            .find(|candidate| high_entropy(candidate.as_str()))
            .map(|candidate| {
                (
                    SecretCategory::HighEntropyToken,
                    candidate.start(),
                    candidate.end(),
                )
            })
    }

    pub(crate) fn configured_count(&self) -> usize {
        self.configured.len()
    }
}

impl fmt::Debug for SecretScanner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretScanner")
            .field("configured_count", &self.configured.len())
            .finish()
    }
}

struct NormalizedText {
    text: String,
    original_byte: Vec<usize>,
    original_len: usize,
    terminal_range: Option<(usize, usize)>,
}

impl NormalizedText {
    fn without_terminal_controls(input: &str) -> Self {
        let mut text = String::with_capacity(input.len());
        let mut original_byte = Vec::with_capacity(input.len());
        let mut ansi_matches = ansi_pattern().find_iter(input).peekable();
        let mut terminal_range = None;
        for (start, character) in input.char_indices() {
            while ansi_matches
                .peek()
                .is_some_and(|matched| matched.end() <= start)
            {
                ansi_matches.next();
            }
            let inside_ansi = ansi_matches
                .peek()
                .is_some_and(|matched| matched.start() <= start && start < matched.end());
            if terminal_range.is_none() {
                if inside_ansi {
                    let matched = ansi_matches.peek().expect("inside a terminal sequence");
                    terminal_range = Some((matched.start(), matched.end()));
                } else if is_terminal_introducer(character) {
                    terminal_range = Some((start, start + character.len_utf8()));
                }
            }
            if inside_ansi || character.is_control() || is_format_character(character) {
                continue;
            }
            text.push(character);
            original_byte.extend((0..character.len_utf8()).map(|offset| start + offset));
        }
        Self {
            text,
            original_byte,
            original_len: input.len(),
            terminal_range,
        }
    }

    fn finding(&self, category: SecretCategory, start: usize, end: usize) -> SecretFinding {
        let original_start = self
            .original_byte
            .get(start)
            .copied()
            .unwrap_or(self.original_len);
        let original_end = end
            .checked_sub(1)
            .and_then(|index| self.original_byte.get(index).copied())
            .map(|index| index.saturating_add(1))
            .unwrap_or(original_start);
        SecretFinding {
            category,
            start: original_start,
            end: original_end,
        }
    }
}

pub(crate) fn is_format_character(character: char) -> bool {
    let mut encoded = [0_u8; 4];
    format_character_pattern().is_match(character.encode_utf8(&mut encoded))
}

fn ansi_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r"(?s:\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07\x1b\u{009c}]*(?:\x07|\x1b\\|\u{009c})|[PX^_].*?(?:\x1b\\|\u{009c})|[ -/]*[0-?@-OQ-WY-Z\\`-~])|\u{009b}[0-?]*[ -/]*[@-~]|\u{009d}[^\x07\u{009c}]*(?:\x07|\x1b\\|\u{009c})|[\u{0090}\u{0098}\u{009e}\u{009f}].*?(?:\x1b\\|\u{009c}))",
        )
        .expect("static ANSI pattern")
    })
}

fn is_terminal_introducer(character: char) -> bool {
    character == '\u{001b}' || ('\u{0080}'..='\u{009f}').contains(&character)
}

fn format_character_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\p{Cf}").expect("static Unicode format pattern"))
}

fn private_key_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)-----BEGIN [^-\r\n]*PRIVATE KEY-----").expect("static private-key pattern")
    })
}

fn authorization_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)authorization\s*:\s*(?:bearer|basic)\s+\S+")
            .expect("static authorization pattern")
    })
}

fn provider_token_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r"(?i)(?:sk[-_][A-Za-z0-9._-]{8,}|ghp_[A-Za-z0-9]{8,}|github_pat_[A-Za-z0-9_]{8,}|xox[baprs]-[A-Za-z0-9-]{8,}|AKIA[A-Z0-9]{12,})",
        )
        .expect("static provider-token pattern")
    })
}

fn high_entropy_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"[A-Za-z0-9+/=_-]{24,}").expect("static high-entropy candidate pattern")
    })
}

fn high_entropy(candidate: &str) -> bool {
    let bytes = candidate.as_bytes();
    if bytes.len() < 24 {
        return false;
    }
    let classes = [
        bytes.iter().any(u8::is_ascii_lowercase),
        bytes.iter().any(u8::is_ascii_uppercase),
        bytes.iter().any(u8::is_ascii_digit),
        bytes
            .iter()
            .any(|byte| matches!(byte, b'+' | b'/' | b'=' | b'_' | b'-')),
    ]
    .into_iter()
    .filter(|present| *present)
    .count();
    if classes < 3 {
        return false;
    }
    let mut counts = BTreeMap::<u8, usize>::new();
    for byte in bytes {
        *counts.entry(*byte).or_default() += 1;
    }
    let length = bytes.len() as f64;
    let entropy = counts.values().fold(0.0, |total, count| {
        let probability = *count as f64 / length;
        total - probability * probability.log2()
    });
    entropy >= 4.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_secret_matching_removes_ansi_controls_and_all_format_characters() {
        let configured = "configured-secret-value-2468";
        let scanner =
            SecretScanner::try_new(vec![SecretString::from(configured)]).expect("valid scanner");
        let (left, right) = configured.split_at(12);

        for separator in [
            "\0",
            "\x1b[31m",
            "\x1b(0",
            "\u{009b}31m",
            "\x1bPignored\x1b\\",
            "\u{0600}",
            "\u{206a}",
            "\u{e0001}",
        ] {
            let candidate = format!("{left}{separator}{right}");
            let finding = scanner
                .scan(&candidate)
                .expect("secret detection takes precedence")
                .expect("configured secret");
            assert_eq!(finding.category, SecretCategory::ConfiguredCredential);
            assert!(finding.end > finding.start);
        }
        assert!(!format!("{scanner:?}").contains(configured));
    }

    #[test]
    fn configured_secret_bounds_fail_without_retaining_input() {
        assert!(
            SecretScanner::try_new(vec![SecretString::from(
                "x".repeat(MAX_CONFIGURED_SECRET_BYTES + 1)
            )])
            .is_err()
        );
        let too_many = (0..=MAX_CONFIGURED_SECRETS)
            .map(|index| SecretString::from(format!("secret-{index}")))
            .collect();
        assert!(SecretScanner::try_new(too_many).is_err());
    }

    #[test]
    fn finding_offsets_map_back_to_the_original_obfuscated_bytes() {
        let scanner = SecretScanner::try_new(vec![SecretString::from("secret-value")])
            .expect("valid scanner");
        let input = "prefix secret\u{0600}-value suffix";
        let finding = scanner
            .scan(input)
            .expect("format characters are normalized")
            .expect("configured secret");

        assert_eq!(finding.category, SecretCategory::ConfiguredCredential);
        assert_eq!(finding.start, 7);
        assert_eq!(finding.end, 21);
        assert_eq!(&input[finding.start..finding.end], "secret\u{0600}-value");
    }

    #[test]
    fn raw_scan_rejects_secrets_inside_terminal_control_payloads() {
        let configured = "configured-secret-value";
        let scanner =
            SecretScanner::try_new(vec![SecretString::from(configured)]).expect("valid scanner");
        let input = format!("\x1b]0;{configured}\x07");

        let finding = scanner
            .scan(&input)
            .expect("raw payload is scanned")
            .expect("configured secret");
        assert_eq!(finding.category, SecretCategory::ConfiguredCredential);
        assert_eq!(&input[finding.start..finding.end], configured);
    }

    #[test]
    fn obfuscated_secrets_inside_terminal_payloads_fail_closed() {
        let configured = "configured-secret-value";
        let scanner =
            SecretScanner::try_new(vec![SecretString::from(configured)]).expect("valid scanner");
        let input = "\x1b]0;configured\u{0600}-secret-value\x07";

        assert!(scanner.scan(input).is_err());
    }

    #[test]
    fn malformed_terminal_sequences_fail_closed() {
        let scanner = SecretScanner::try_new(Vec::new()).expect("valid scanner");
        let error = scanner
            .scan("ordinary\x1b[31")
            .expect_err("terminal obfuscation");

        assert_eq!(error.start, 8);
        assert_eq!(error.end, 9);
    }

    #[test]
    fn authorization_detection_preserves_raw_whitespace_semantics() {
        let scanner = SecretScanner::try_new(Vec::new()).expect("valid scanner");
        for candidate in [
            "Authorization: Bearer\nhidden-value",
            "Authorization: Basic\thidden-value",
        ] {
            let finding = scanner
                .scan(candidate)
                .expect("raw whitespace is supported")
                .expect("authorization secret");
            assert_eq!(finding.category, SecretCategory::AuthorizationHeader);
        }
    }

    #[test]
    fn public_secret_categories_remain_exhaustively_matchable() {
        fn category_code(category: SecretCategory) -> u8 {
            match category {
                SecretCategory::ConfiguredCredential => 1,
                SecretCategory::PrivateKey => 2,
                SecretCategory::AuthorizationHeader => 3,
                SecretCategory::ProviderToken => 4,
                SecretCategory::HighEntropyToken => 5,
            }
        }

        assert_eq!(category_code(SecretCategory::ConfiguredCredential), 1);
    }
}
