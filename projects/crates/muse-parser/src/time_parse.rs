//! Parse timeline time literals with optional unit suffixes.
use crate::ast::{TimePoint, TimeUnit};

/// Parse `8`, `8.5`, `8bar`, `8bars`, `8b`, `8beat`, `8beats`, `12.5s`.
///
/// Bare numbers are [`TimeUnit::Bars`] (compat with existing scripts).
pub fn parse_time_point(raw: &str) -> Option<TimePoint> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let lower = raw.to_ascii_lowercase();
    let (number, unit) = if let Some(rest) = lower.strip_suffix("bars") {
        (rest, TimeUnit::Bars)
    } else if let Some(rest) = lower.strip_suffix("bar") {
        (rest, TimeUnit::Bars)
    } else if let Some(rest) = lower.strip_suffix("beats") {
        (rest, TimeUnit::Beats)
    } else if let Some(rest) = lower.strip_suffix("beat") {
        (rest, TimeUnit::Beats)
    } else if let Some(rest) = lower.strip_suffix('b') {
        // `b` = beats; do not treat as bars (`bar` already handled).
        (rest, TimeUnit::Beats)
    } else if let Some(rest) = lower.strip_suffix('s') {
        (rest, TimeUnit::Seconds)
    } else {
        (lower.as_str(), TimeUnit::Bars)
    };
    let number = number.trim();
    if number.is_empty() {
        return None;
    }
    let value: f64 = number.parse().ok()?;
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    Some(TimePoint { value, unit })
}

/// Parse `start..end`; both ends must share the same unit dimension.
pub fn parse_time_range(raw: &str) -> Option<(TimePoint, TimePoint)> {
    let (start_raw, end_raw) = raw.split_once("..")?;
    let start = parse_time_point(start_raw)?;
    let end = parse_time_point(end_raw)?;
    if start.unit != end.unit {
        return None;
    }
    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_number_is_bars() {
        let p = parse_time_point("8").unwrap();
        assert_eq!(p.value, 8.0);
        assert_eq!(p.unit, TimeUnit::Bars);
    }

    #[test]
    fn suffixes() {
        assert_eq!(parse_time_point("12.5s").unwrap().unit, TimeUnit::Seconds);
        assert_eq!(parse_time_point("4bar").unwrap().unit, TimeUnit::Bars);
        assert_eq!(parse_time_point("8b").unwrap().unit, TimeUnit::Beats);
        assert_eq!(parse_time_point("8beats").unwrap().value, 8.0);
    }

    #[test]
    fn range_rejects_mixed_units() {
        assert!(parse_time_range("0s..8bar").is_none());
        let (a, b) = parse_time_range("0..8").unwrap();
        assert_eq!(a.unit, TimeUnit::Bars);
        assert_eq!(b.value, 8.0);
    }
}
