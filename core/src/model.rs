use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Service {
    Claude,
    Codex,
    Grok,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum WindowKind {
    /// 短時間制限(5時間窓など)
    ShortTerm,
    /// 週間制限
    Weekly,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Window {
    pub kind: WindowKind,
    /// 消費率 0.0..=100.0
    pub used_percent: f64,
    pub resets_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Status {
    Ok,
    /// 最新取得に失敗し、前回値を表示している
    Stale { reason: String },
    /// 取得不可(値なし)
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ServiceUsage {
    pub service: Service,
    pub windows: Vec<Window>,
    pub fetched_at: DateTime<Utc>,
    pub status: Status,
}

impl ServiceUsage {
    pub fn unavailable(service: Service, now: DateTime<Utc>, reason: impl Into<String>) -> Self {
        Self {
            service,
            windows: vec![],
            fetched_at: now,
            status: Status::Unavailable { reason: reason.into() },
        }
    }
}

/// 0..100 に丸める。NaN は 0 扱い。
pub fn clamp_percent(v: f64) -> f64 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 100.0)
    }
}
