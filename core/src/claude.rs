//! Claude (Claude Code サブスク) の使用量。
//! GET https://api.anthropic.com/api/oauth/usage (非公式・読み取り専用)
//!
//! 想定レスポンス(コミュニティ製ツールの利用例に基づく。実機で要検証):
//! { "five_hour": {"utilization": 68.0, "resets_at": "2026-10-18T08:13:00+00:00"},
//!   "seven_day": {"utilization": 41.0, "resets_at": "..."}, ... }

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::error::FetchError;
use crate::model::{clamp_percent, Window, WindowKind};

pub const URL: &str = "https://api.anthropic.com/api/oauth/usage";

#[derive(Deserialize)]
struct Raw {
    five_hour: Option<RawWindow>,
    seven_day: Option<RawWindow>,
}

#[derive(Deserialize)]
struct RawWindow {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

fn conv(kind: WindowKind, w: RawWindow) -> Option<Window> {
    let used = w.utilization?;
    let resets_at = w
        .resets_at
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|d| d.with_timezone(&Utc));
    Some(Window { kind, used_percent: clamp_percent(used), resets_at })
}

pub fn parse(body: &str) -> Result<Vec<Window>, FetchError> {
    let raw: Raw = serde_json::from_str(body).map_err(|e| FetchError::Parse(e.to_string()))?;
    let mut out = vec![];
    if let Some(w) = raw.five_hour.and_then(|w| conv(WindowKind::ShortTerm, w)) {
        out.push(w);
    }
    if let Some(w) = raw.seven_day.and_then(|w| conv(WindowKind::Weekly, w)) {
        out.push(w);
    }
    if out.is_empty() {
        return Err(FetchError::Parse("使用量フィールドがありません".into()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fixture() {
        let body = include_str!("../tests/fixtures/claude_usage.json");
        let w = parse(body).unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].kind, WindowKind::ShortTerm);
        assert_eq!(w[0].used_percent, 68.0);
        assert!(w[0].resets_at.is_some());
        assert_eq!(w[1].kind, WindowKind::Weekly);
        assert_eq!(w[1].used_percent, 41.5);
    }

    #[test]
    fn null_reset_is_ok() {
        let w = parse(r#"{"five_hour":{"utilization":0.0,"resets_at":null}}"#).unwrap();
        assert_eq!(w[0].resets_at, None);
    }

    #[test]
    fn missing_fields_degrade_to_error() {
        assert!(matches!(parse("{}"), Err(FetchError::Parse(_))));
        assert!(matches!(parse("not json"), Err(FetchError::Parse(_))));
        assert!(matches!(
            parse(r#"{"five_hour":{"utilization":"x"}}"#),
            Err(FetchError::Parse(_))
        ));
    }

    #[test]
    fn clamps_out_of_range() {
        let w = parse(r#"{"five_hour":{"utilization":130.0,"resets_at":null}}"#).unwrap();
        assert_eq!(w[0].used_percent, 100.0);
    }
}
