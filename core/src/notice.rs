//! 閾値超過・リセットの通知判定(純粋ロジック)。

use std::collections::HashSet;

use crate::model::{Service, ServiceUsage, Status, WindowKind};

pub const THRESHOLDS: [u8; 2] = [80, 95];

#[derive(Debug, PartialEq)]
pub struct Notice {
    pub title: String,
    pub body: String,
}

#[derive(Default)]
pub struct Tracker {
    fired: HashSet<(u8, u8, u8)>, // (service, kind, threshold)
}

fn svc_id(s: Service) -> u8 {
    s as u8
}
fn kind_id(k: WindowKind) -> u8 {
    k as u8
}
fn svc_name(s: Service) -> &'static str {
    match s {
        Service::Claude => "Claude",
        Service::Codex => "Codex",
        Service::Grok => "Grok",
    }
}
fn kind_name(k: WindowKind) -> &'static str {
    match k {
        WindowKind::ShortTerm => "5時間",
        WindowKind::Weekly => "週間",
    }
}

impl Tracker {
    /// 最新の取得結果を渡す。Ok のときだけ判定(古い値では通知しない)。
    pub fn update(&mut self, u: &ServiceUsage) -> Vec<Notice> {
        let mut out = vec![];
        if u.status != Status::Ok {
            return out;
        }
        for w in &u.windows {
            let mut reset_candidate = false;
            for th in THRESHOLDS {
                let key = (svc_id(u.service), kind_id(w.kind), th);
                if w.used_percent >= th as f64 {
                    if self.fired.insert(key) {
                        out.push(Notice {
                            title: format!("{} {}の使用量が{}%を超えました", svc_name(u.service), kind_name(w.kind), th),
                            body: format!("現在 {}%", w.used_percent.round()),
                        });
                    }
                } else if self.fired.remove(&key) {
                    reset_candidate = true;
                }
            }
            if reset_candidate {
                out.push(Notice {
                    title: format!("{} {}の使用量が下がりました", svc_name(u.service), kind_name(w.kind)),
                    body: format!("現在 {}%(リセットされた可能性があります)", w.used_percent.round()),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Window;
    use chrono::{TimeZone, Utc};

    fn u(p: f64, status: Status) -> ServiceUsage {
        ServiceUsage {
            service: Service::Claude,
            windows: vec![Window { kind: WindowKind::ShortTerm, used_percent: p, resets_at: None }],
            fetched_at: Utc.with_ymd_and_hms(2026, 10, 18, 9, 0, 0).unwrap(),
            status,
        }
    }

    #[test]
    fn fires_once_per_threshold() {
        let mut t = Tracker::default();
        assert!(t.update(&u(50.0, Status::Ok)).is_empty());
        assert_eq!(t.update(&u(82.0, Status::Ok)).len(), 1);
        assert!(t.update(&u(85.0, Status::Ok)).is_empty());
        assert_eq!(t.update(&u(96.0, Status::Ok)).len(), 1); // 95%のみ新規
        assert!(t.update(&u(97.0, Status::Ok)).is_empty());
    }

    #[test]
    fn reset_notifies_and_rearms() {
        let mut t = Tracker::default();
        t.update(&u(96.0, Status::Ok)); // 80, 95 両方発火
        let n = t.update(&u(3.0, Status::Ok));
        assert_eq!(n.len(), 1);
        assert!(n[0].title.contains("下がりました"));
        assert_eq!(t.update(&u(81.0, Status::Ok)).len(), 1); // 再度80%超で再通知
    }

    #[test]
    fn stale_never_notifies() {
        let mut t = Tracker::default();
        assert!(t.update(&u(99.0, Status::Stale { reason: "x".into() })).is_empty());
        assert!(t.update(&u(99.0, Status::Unavailable { reason: "x".into() })).is_empty());
    }
}
