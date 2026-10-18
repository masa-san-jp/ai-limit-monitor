use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Remaining {
    pub days: i64,
    pub hours: i64,
    pub minutes: i64,
}

/// リセットまでの残り時間。既に過ぎている場合は None。
/// 秒は切り上げて分に含める(0分表示で残り数秒、を避ける)。
pub fn remaining(resets_at: DateTime<Utc>, now: DateTime<Utc>) -> Option<Remaining> {
    let secs = (resets_at - now).num_seconds();
    if secs <= 0 {
        return None;
    }
    let total_min = (secs + 59) / 60;
    Some(Remaining {
        days: total_min / (60 * 24),
        hours: (total_min / 60) % 24,
        minutes: total_min % 60,
    })
}

/// 例: "3日 5時間" / "1時間42分" / "12分" / "リセット済み"
pub fn format_ja(resets_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> String {
    let Some(at) = resets_at else {
        return "リセット時刻不明".to_string();
    };
    match remaining(at, now) {
        None => "リセット済み".to_string(),
        Some(r) if r.days > 0 => format!("{}日 {}時間", r.days, r.hours),
        Some(r) if r.hours > 0 => format!("{}時間{:02}分", r.hours, r.minutes),
        Some(r) => format!("{}分", r.minutes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
    }

    #[test]
    fn days_hours() {
        let now = t(2026, 10, 18, 6, 31, 0);
        let at = t(2026, 10, 21, 11, 31, 0);
        assert_eq!(remaining(at, now), Some(Remaining { days: 3, hours: 5, minutes: 0 }));
        assert_eq!(format_ja(Some(at), now), "3日 5時間");
    }

    #[test]
    fn hours_minutes() {
        let now = t(2026, 10, 18, 6, 31, 0);
        let at = t(2026, 10, 18, 8, 13, 0);
        assert_eq!(format_ja(Some(at), now), "1時間42分");
    }

    #[test]
    fn seconds_round_up() {
        let now = t(2026, 10, 18, 6, 31, 0);
        let at = t(2026, 10, 18, 6, 31, 5);
        assert_eq!(format_ja(Some(at), now), "1分");
    }

    #[test]
    fn crosses_month_boundary() {
        let now = t(2026, 10, 30, 23, 0, 0);
        let at = t(2026, 11, 2, 0, 0, 0);
        assert_eq!(remaining(at, now), Some(Remaining { days: 2, hours: 1, minutes: 0 }));
    }

    #[test]
    fn already_passed_and_unknown() {
        let now = t(2026, 10, 18, 6, 31, 0);
        assert_eq!(format_ja(Some(t(2026, 10, 18, 6, 0, 0)), now), "リセット済み");
        assert_eq!(format_ja(Some(now), now), "リセット済み");
        assert_eq!(format_ja(None, now), "リセット時刻不明");
    }
}
