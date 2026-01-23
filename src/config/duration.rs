//! Custom serde module for parsing duration strings like "30s", "5m", "1h".

use serde::{self, Deserialize, Deserializer};
use std::time::Duration;

pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        Some(s) => parse_duration(&s).map_err(serde::de::Error::custom),
        None => Ok(Duration::ZERO),
    }
}

pub(crate) fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(Duration::ZERO);
    }

    // Find where the number ends and the unit begins
    let num_end = s
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(s.len());

    let (num_str, unit) = s.split_at(num_end);
    let num: f64 = num_str
        .parse()
        .map_err(|_| format!("invalid duration number: {}", num_str))?;

    let multiplier = match unit.trim() {
        "ns" => 1e-9,
        "us" | "µs" => 1e-6,
        "ms" => 1e-3,
        "s" | "" => 1.0,
        "m" => 60.0,
        "h" => 3600.0,
        _ => return Err(format!("unknown duration unit: {}", unit)),
    };

    Ok(Duration::from_secs_f64(num * multiplier))
}
