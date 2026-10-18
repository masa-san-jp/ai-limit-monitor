//! UI に渡す表示用データ(純粋関数。Tauri 非依存でテスト可能)。

use chrono::{DateTime, FixedOffset, Utc};
use serde::Serialize;

use crate::countdown::format_ja;
use crate::model::{Service, ServiceUsage, Status, WindowKind};

#[derive(Debug, Serialize, PartialEq)]
pub struct WindowView {
    pub label: String,
    pub used_percent: f64,
    pub remaining_text: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ServiceView {
    pub service: String,
    /// "ok" | "stale" | "unavailable"
    pub status: String,
    pub fetched_at_label: String,
    pub message: Option<String>,
    pub windows: Vec<WindowView>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Snapshot {
    pub services: Vec<ServiceView>,
    /// メニューバー表示用: 全サービス・全窓の最大消費率
    pub max_percent: Option<f64>,
    pub updated_label: String,
}

const ORDER: [Service; 3] = [Service::Claude, Service::Codex, Service::Grok];

fn name(s: Service) -> &'static str {
    match s {
        Service::Claude => "Claude",
        Service::Codex => "Codex",
        Service::Grok => "Grok",
    }
}

fn kind_label(k: WindowKind) -> &'static str {
    match k {
        WindowKind::ShortTerm => "5時間",
        WindowKind::Weekly => "週間",
    }
}

fn hm(t: DateTime<Utc>, tz: FixedOffset) -> String {
    t.with_timezone(&tz).format("%H:%M").to_string()
}

pub fn build(usages: &[ServiceUsage], now: DateTime<Utc>, tz: FixedOffset) -> Snapshot {
    let mut services = vec![];
    let mut max: Option<f64> = None;
    for s in ORDER {
        let u = usages.iter().find(|u| u.service == s);
        let view = match u {
            None => ServiceView {
                service: name(s).into(),
                status: "unavailable".into(),
                fetched_at_label: "--".into(),
                message: Some("未取得".into()),
                windows: vec![],
            },
            Some(u) => {
                let (status, message) = match &u.status {
                    Status::Ok => ("ok", None),
                    Status::Stale { reason } => ("stale", Some(reason.clone())),
                    Status::Unavailable { reason } => ("unavailable", Some(reason.clone())),
                };
                let windows: Vec<WindowView> = u
                    .windows
                    .iter()
                    .map(|w| WindowView {
                        label: kind_label(w.kind).into(),
                        used_percent: w.used_percent,
                        remaining_text: format_ja(w.resets_at, now),
                    })
                    .collect();
                for w in &u.windows {
                    max = Some(max.map_or(w.used_percent, |m| m.max(w.used_percent)));
                }
                ServiceView {
                    service: name(s).into(),
                    status: status.into(),
                    fetched_at_label: hm(u.fetched_at, tz),
                    message,
                    windows,
                }
            }
        };
        services.push(view);
    }
    Snapshot { services, max_percent: max, updated_label: hm(now, tz) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Window;
    use chrono::TimeZone;

    fn jst() -> FixedOffset {
        FixedOffset::east_opt(9 * 3600).unwrap()
    }

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 18, 9, 24, 0).unwrap() // 18:24 JST
    }

    #[test]
    fn builds_all_three_in_order_with_max() {
        let claude = ServiceUsage {
            service: Service::Claude,
            windows: vec![
                Window { kind: WindowKind::ShortTerm, used_percent: 68.0, resets_at: Some(now() + chrono::Duration::minutes(102)) },
                Window { kind: WindowKind::Weekly, used_percent: 41.0, resets_at: Some(now() + chrono::Duration::hours(77)) },
            ],
            fetched_at: now(),
            status: Status::Ok,
        };
        let s = build(&[claude], now(), jst());
        assert_eq!(s.services.iter().map(|v| v.service.as_str()).collect::<Vec<_>>(), ["Claude", "Codex", "Grok"]);
        assert_eq!(s.max_percent, Some(68.0));
        assert_eq!(s.updated_label, "18:24");
        assert_eq!(s.services[0].windows[0].label, "5時間");
        assert_eq!(s.services[0].windows[0].remaining_text, "1時間42分");
        assert_eq!(s.services[0].windows[1].remaining_text, "3日 5時間");
        assert_eq!(s.services[1].status, "unavailable");
    }

    #[test]
    fn stale_shows_previous_fetch_time_in_local_tz() {
        let t = Utc.with_ymd_and_hms(2026, 10, 18, 8, 58, 0).unwrap(); // 17:58 JST
        let u = ServiceUsage {
            service: Service::Grok,
            windows: vec![Window { kind: WindowKind::Weekly, used_percent: 30.0, resets_at: None }],
            fetched_at: t,
            status: Status::Stale { reason: "再ログインが必要です".into() },
        };
        let s = build(&[u], now(), jst());
        let g = &s.services[2];
        assert_eq!(g.status, "stale");
        assert_eq!(g.fetched_at_label, "17:58");
        assert_eq!(g.message.as_deref(), Some("再ログインが必要です"));
        assert_eq!(g.windows[0].remaining_text, "リセット時刻不明");
    }

    #[test]
    fn no_data_means_no_max() {
        assert_eq!(build(&[], now(), jst()).max_percent, None);
    }
}
