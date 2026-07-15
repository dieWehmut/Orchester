use std::sync::OnceLock;

use regex::Regex;

use super::EvolutionError;

pub(super) const MAX_IDENTIFIER_BYTES: usize = 256;
pub(super) const MAX_FORMAT_BYTES: usize = 64;
const MAX_TIMESTAMP_BYTES: usize = 64;

pub(super) fn validate_text(value: &str, max_bytes: usize) -> Result<String, EvolutionError> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > max_bytes
        || value.chars().any(char::is_control)
        || value
            .chars()
            .any(|character| matches!(character, '\u{2028}' | '\u{2029}'))
        || format_character_pattern().is_match(value)
    {
        return Err(EvolutionError::InvalidInput);
    }
    Ok(value.to_owned())
}

pub(super) fn validate_timestamp(value: &str) -> Result<String, EvolutionError> {
    timestamp_order_key(value)?;
    if value.as_bytes().get(19) != Some(&b'.') {
        return Ok(value.to_owned());
    }
    let fraction = value
        .get(20..value.len() - 1)
        .ok_or(EvolutionError::InvalidTimestamp)?
        .trim_end_matches('0');
    if fraction.is_empty() {
        Ok(format!("{}Z", &value[..19]))
    } else {
        Ok(format!("{}.{fraction}Z", &value[..19]))
    }
}

pub(super) fn expiry_is_later(created_at: &str, expires_at: &str) -> Result<bool, EvolutionError> {
    Ok(timestamp_order_key(expires_at)? > timestamp_order_key(created_at)?)
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct TimestampOrderKey {
    year: u32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    nanos: u32,
}

fn timestamp_order_key(value: &str) -> Result<TimestampOrderKey, EvolutionError> {
    if value.len() > MAX_TIMESTAMP_BYTES || !timestamp_pattern().is_match(value) {
        return Err(EvolutionError::InvalidTimestamp);
    }
    let year = timestamp_part(value, 0, 4)?;
    let month = timestamp_part(value, 5, 7)?;
    let day = timestamp_part(value, 8, 10)?;
    let hour = timestamp_part(value, 11, 13)?;
    let minute = timestamp_part(value, 14, 16)?;
    let second = timestamp_part(value, 17, 19)?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return Err(EvolutionError::InvalidTimestamp),
    };
    if day == 0 || day > max_day || hour > 23 || minute > 59 || second > 59 {
        return Err(EvolutionError::InvalidTimestamp);
    }
    let fraction = value
        .get(20..value.len() - 1)
        .map(|part| format!("{part:0<9}"))
        .unwrap_or_else(|| "000000000".to_owned());
    let nanos = fraction
        .parse::<u32>()
        .map_err(|_| EvolutionError::InvalidTimestamp)?;
    Ok(TimestampOrderKey {
        year,
        month,
        day,
        hour,
        minute,
        second,
        nanos,
    })
}

fn timestamp_part(value: &str, start: usize, end: usize) -> Result<u32, EvolutionError> {
    value
        .get(start..end)
        .and_then(|part| part.parse().ok())
        .ok_or(EvolutionError::InvalidTimestamp)
}

fn timestamp_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?Z$")
            .expect("static evolution timestamp pattern")
    })
}

fn format_character_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\p{Cf}").expect("static Unicode format pattern"))
}
