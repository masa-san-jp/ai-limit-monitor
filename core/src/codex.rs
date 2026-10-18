//! Codex (ChatGPT サブスク) の使用量。
//! GET https://chatgpt.com/backend-api/wham/usage (非公式・読み取り専用)
//!
//! 想定レスポンス(コミュニティ製ツールの利用例に基づく。実機で要検証):
//! { "rate_limit": {
//!     "primary_window":   {"used_percent": 17, "limit_window_seconds": 18000, "reset_after_seconds": 11280, "reset_at": 1789000000},
//!     "secondary_window": {"used_percent": 52, "limit_window_seconds": 604800, "reset_after_seconds": 390000, "reset_at": 1789300000} } }

use chrono::{DateTime, Duration, TimeZone, Utc};
use serde::Deserialize;

use crate::error::FetchError;
use crate::model::{clamp_percent, Window, WindowKind};

pub const URL: &str = "https://chatgpt.com/backend-api/wham/usage";

#[derive(Deserialize)]
struct Raw {
    rate_limit: Option<RateLimit>,
}

#[derive(Deserialize)]
struct RateLimit {
    primary_window: Option<RawWindow>,
    secondary_window: Option<RawWindow>,
}

#[derive(Deserialize)]
struct RawWindow {
    used_percent: Option<f64>,
    limit_window_seconds: Option<i64>,
    reset_after_seconds: Option<i64>,
    reset_at: Option<i64>,
}

/// 窓長が分かればそれで短時間/週間を判定。不明なら位置(primary=短時間, secondary=週間)。
fn classify(w: &RawWindow, fallback: WindowKind) -> WindowKind {
    match w.limit_window_seconds {
        Some(s) if s <= 6 * 3600 => WindowKind::ShortTerm,
        Some(_) => WindowKind::Weekly,
        None => fallback,
    }
}

fn conv(w: RawWindow, fallback: WindowKind, now: DateTime<Utc>) -> Option<Window> {
    let used = w.used_percent?;
    let kind = classify(&w, fallback);
    let resets_at = match (w.reset_at, w.reset_after_seconds) {
        (Some(ts), _) => Utc.timestamp_opt(ts, 0).single(),
        (None, Some(after)) => Some(now + Duration::seconds(after)),
        _ => None,
    };
    Some(Window { kind, used_percent: clamp_percent(used), resets_at })
}

pub fn parse(body: &str, now: DateTime<Utc>) -> Result<Vec<Window>, FetchError> {
    let raw: Raw = serde_json::from_str(body).map_err(|e| FetchError::Parse(e.to_string()))?;
    let rl = raw
        .rate_limit
        .ok_or_else(|| FetchError::Parse("rate_limit がありません".into()))?;
    let mut out = vec![];
    if let Some(w) = rl.primary_window.and_then(|w| conv(w, WindowKind::ShortTerm, now)) {
        out.push(w);
    }
    if let Some(w) = rl.secondary_window.and_then(|w| conv(w, WindowKind::Weekly, now)) {
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

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 18, 6, 31, 0).unwrap()
    }

    #[test]
    fn parses_fixture() {
        let body = include_str!("../tests/fixtures/codex_usage.json");
        let w = parse(body, now()).unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].kind, WindowKind::ShortTerm);
        assert_eq!(w[0].used_percent, 17.0);
        assert_eq!(w[1].kind, WindowKind::Weekly);
        assert_eq!(w[1].used_percent, 52.0);
        // reset_at(絶対時刻)を優先
        assert_eq!(w[0].resets_at, Utc.timestamp_opt(1792300000, 0).single());
    }

    #[test]
    fn falls_back_to_reset_after_seconds() {
        let body = r#"{"rate_limit":{"primary_window":{"used_percent":5,"reset_after_seconds":600}}}"#;
        let w = parse(body, now()).unwrap();
        assert_eq!(w[0].resets_at, Some(now() + Duration::seconds(600)));
        assert_eq!(w[0].kind, WindowKind::ShortTerm);
    }

    #[test]
    fn classifies_by_window_length() {
        // primary が週間の窓長を持つプランでも誤分類しない
        let body = r#"{"rate_limit":{"primary_window":{"used_percent":5,"limit_window_seconds":604800}}}"#;
        let w = parse(body, now()).unwrap();
        assert_eq!(w[0].kind, WindowKind::Weekly);
    }

    #[test]
    fn missing_fields_degrade_to_error() {
        assert!(matches!(parse("{}", now()), Err(FetchError::Parse(_))));
        assert!(matches!(parse(r#"{"rate_limit":{}}"#, now()), Err(FetchError::Parse(_))));
        assert!(matches!(parse("<html>", now()), Err(FetchError::Parse(_))));
    }
}
