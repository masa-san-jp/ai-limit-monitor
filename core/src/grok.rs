//! Grok (SuperGrok) の使用量。消費者向けの公開APIは確認できていないため、
//! アプリ専用プロファイルの WebView で 統合プランの grok.com/dashboard の Usage を開き、注入スクリプトが
//! 画面から抽出した JSON をここで解釈する。
//!
//! 注入スクリプトが返す想定ペイロード(DOM のセレクタは実アカウントで要較正):
//! { "used_percent": 30, "resets_at": "2026-10-20T08:00:00Z" }
//! または resets_at が取れず相対表記のみの場合:
//! { "used_percent": 30, "reset_text": "2日 3時間" | "2 days 3 hours" | "5 hours" }

use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;

use crate::error::FetchError;
use crate::model::{clamp_percent, Window, WindowKind};

pub const USAGE_PAGE: &str = "https://grok.com/dashboard";

#[derive(Deserialize)]
struct Raw {
    used_percent: Option<f64>,
    resets_at: Option<String>,
    reset_text: Option<String>,
}

/// "2日 3時間" / "2 days 3 hours 10 minutes" / "45分" などを Duration に。
pub fn parse_relative(text: &str) -> Option<Duration> {
    let mut total = 0i64;
    let mut found = false;
    let mut num = String::new();
    let mut word = String::new();
    let chars: Vec<char> = text.chars().chain(std::iter::once(' ')).collect();
    let mut pending: Option<i64> = None;

    let apply = |n: i64, unit: &str| -> Option<i64> {
        let u = unit.to_lowercase();
        if u.starts_with('日') || u.starts_with("day") || u == "d" {
            Some(n * 86400)
        } else if u.starts_with("時間") || u.starts_with("hour") || u == "h" || u == "hr" || u == "hrs" {
            Some(n * 3600)
        } else if u.starts_with('分') || u.starts_with("min") || u == "m" {
            Some(n * 60)
        } else {
            None
        }
    };

    for c in chars {
        if c.is_ascii_digit() {
            if !word.is_empty() {
                if let Some(n) = pending.take() {
                    if let Some(s) = apply(n, &word) {
                        total += s;
                        found = true;
                    }
                }
                word.clear();
            }
            num.push(c);
        } else if c.is_whitespace() {
            if !num.is_empty() {
                pending = num.parse().ok();
                num.clear();
            }
            if !word.is_empty() {
                if let Some(n) = pending.take() {
                    if let Some(s) = apply(n, &word) {
                        total += s;
                        found = true;
                    }
                }
                word.clear();
            }
        } else {
            if !num.is_empty() {
                pending = num.parse().ok();
                num.clear();
            }
            word.push(c);
        }
    }
    if found { Some(Duration::seconds(total)) } else { None }
}

pub fn parse(body: &str, now: DateTime<Utc>) -> Result<Vec<Window>, FetchError> {
    let raw: Raw = serde_json::from_str(body).map_err(|e| FetchError::Parse(e.to_string()))?;
    let used = raw
        .used_percent
        .ok_or_else(|| FetchError::Parse("used_percent がありません".into()))?;
    if !used.is_finite() || !(0.0..=100.0).contains(&used) {
        return Err(FetchError::Parse("使用率が範囲外です".into()));
    }
    let resets_at = raw
        .resets_at
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|d| d.with_timezone(&Utc))
        .or_else(|| raw.reset_text.as_deref().and_then(parse_relative).map(|d| now + d));
    // The X Subscription の共有週間枠。旧 Cursor の月間枠は対象外。
    Ok(vec![Window { kind: WindowKind::Weekly, used_percent: clamp_percent(used), resets_at }])
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 18, 6, 31, 0).unwrap()
    }

    #[test]
    fn absolute_reset() {
        let w = parse(r#"{"used_percent":30,"resets_at":"2026-10-20T08:00:00Z"}"#, now()).unwrap();
        assert_eq!(w[0].kind, WindowKind::Weekly);
        assert_eq!(w[0].resets_at, Utc.with_ymd_and_hms(2026, 10, 20, 8, 0, 0).single());
    }

    #[test]
    fn relative_reset_text() {
        assert_eq!(parse_relative("2日 3時間"), Some(Duration::seconds(2 * 86400 + 3 * 3600)));
        assert_eq!(parse_relative("2 days 3 hours"), Some(Duration::seconds(2 * 86400 + 3 * 3600)));
        assert_eq!(parse_relative("45分"), Some(Duration::minutes(45)));
        assert_eq!(parse_relative("1 hour 10 minutes"), Some(Duration::minutes(70)));
        assert_eq!(parse_relative("soon"), None);
        let w = parse(r#"{"used_percent":5,"reset_text":"5 hours"}"#, now()).unwrap();
        assert_eq!(w[0].resets_at, Some(now() + Duration::hours(5)));
    }

    #[test]
    fn invalid_percent_is_rejected() {
        for body in [r#"{"used_percent":-1}"#, r#"{"used_percent":101}"#] {
            assert!(parse(body, now()).is_err());
        }
    }

    #[test]
    fn missing_fields_degrade() {
        assert!(matches!(parse("{}", now()), Err(FetchError::Parse(_))));
        assert!(matches!(parse("<html>", now()), Err(FetchError::Parse(_))));
    }
}
