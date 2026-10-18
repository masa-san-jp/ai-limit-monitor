//! 取得オーケストレーション。HTTP 実装は trait で差し替え(本体は reqwest 等、テストはモック)。
//! 失敗時は前回値を Stale として残す(取得時刻を明示)。

use chrono::{DateTime, Utc};

use crate::allowlist::is_allowed;
use crate::claude;
use crate::codex;
use crate::creds::Credential;
use crate::error::FetchError;
use crate::model::{Service, ServiceUsage, Status, Window};
use crate::secret::redact;

pub struct HttpResponse {
    pub status: u16,
    pub body: String,
    pub retry_after_secs: Option<u64>,
}

pub trait HttpGet {
    /// headers は (名前, 値)。実装側はログにヘッダ値を出さないこと。
    fn get(&self, url: &str, headers: &[(String, String)]) -> Result<HttpResponse, FetchError>;
}

fn request(
    http: &dyn HttpGet,
    url: &str,
    headers: Vec<(String, String)>,
) -> Result<String, FetchError> {
    if !is_allowed(url) {
        return Err(FetchError::NotAllowed);
    }
    let res = http.get(url, &headers)?;
    match res.status {
        200..=299 => Ok(res.body),
        401 | 403 => Err(FetchError::NeedsRelogin(format!("認証エラー({})", res.status))),
        429 => Err(FetchError::RateLimited { retry_after_secs: res.retry_after_secs }),
        s => Err(FetchError::Network(format!("HTTP {s}"))),
    }
}

pub fn fetch_claude(http: &dyn HttpGet, cred: &Credential, _now: DateTime<Utc>) -> Result<Vec<Window>, FetchError> {
    let body = request(
        http,
        claude::URL,
        vec![
            ("Authorization".into(), format!("Bearer {}", cred.token.expose())),
            ("anthropic-beta".into(), "oauth-2025-04-20".into()),
            ("Accept".into(), "application/json".into()),
        ],
    )?;
    claude::parse(&body)
}

pub fn fetch_codex(http: &dyn HttpGet, cred: &Credential, now: DateTime<Utc>) -> Result<Vec<Window>, FetchError> {
    let mut h = vec![
        ("Authorization".into(), format!("Bearer {}", cred.token.expose())),
        ("Accept".into(), "application/json".into()),
    ];
    if let Some(id) = &cred.account_id {
        h.push(("chatgpt-account-id".into(), id.clone()));
    }
    let body = request(http, codex::URL, h)?;
    codex::parse(&body, now)
}

/// 取得結果と前回値から、表示用の ServiceUsage を組み立てる。
/// 失敗時は前回値を Stale で残し、前回値がなければ Unavailable。
/// エラー文言は redact を通す(トークン混入の保険)。
pub fn resolve(
    service: Service,
    prev: Option<&ServiceUsage>,
    result: Result<Vec<Window>, FetchError>,
    now: DateTime<Utc>,
) -> ServiceUsage {
    match result {
        Ok(windows) => ServiceUsage { service, windows, fetched_at: now, status: Status::Ok },
        Err(e) => {
            let reason = redact(&e.to_string());
            match prev {
                Some(p) if !p.windows.is_empty() => ServiceUsage {
                    service,
                    windows: p.windows.clone(),
                    fetched_at: p.fetched_at,
                    status: Status::Stale { reason },
                },
                _ => ServiceUsage::unavailable(service, now, reason),
            }
        }
    }
}

/// 429 時の次回ポーリングまでの待ち秒数。指数バックオフ(上限1時間)、Retry-After があれば優先。
/// base_secs はポーリング間隔(最小300秒を強制)。
pub fn next_delay_secs(base_secs: u64, consecutive_failures: u32, retry_after: Option<u64>) -> u64 {
    let base = base_secs.max(300);
    if let Some(ra) = retry_after {
        return ra.clamp(base, 3600);
    }
    let shift = consecutive_failures.min(6);
    (base << shift).min(3600)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::WindowKind;
    use crate::secret::Secret;
    use chrono::TimeZone;
    use std::cell::RefCell;

    struct Mock {
        status: u16,
        body: String,
        seen: RefCell<Vec<(String, Vec<(String, String)>)>>,
    }

    impl Mock {
        fn new(status: u16, body: &str) -> Self {
            Mock { status, body: body.into(), seen: RefCell::new(vec![]) }
        }
    }

    impl HttpGet for Mock {
        fn get(&self, url: &str, headers: &[(String, String)]) -> Result<HttpResponse, FetchError> {
            self.seen.borrow_mut().push((url.into(), headers.to_vec()));
            Ok(HttpResponse { status: self.status, body: self.body.clone(), retry_after_secs: None })
        }
    }

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 18, 6, 31, 0).unwrap()
    }

    fn cred() -> Credential {
        Credential { token: Secret::new("tok-abcdefghijklmnopqrstuvwxyz"), account_id: Some("acct_1".into()) }
    }

    #[test]
    fn claude_success_sends_bearer_to_allowed_host_only() {
        let m = Mock::new(200, include_str!("../tests/fixtures/claude_usage.json"));
        let w = fetch_claude(&m, &cred(), now()).unwrap();
        assert_eq!(w.len(), 2);
        let seen = m.seen.borrow();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, "https://api.anthropic.com/api/oauth/usage");
    }

    #[test]
    fn codex_sends_account_id() {
        let m = Mock::new(200, include_str!("../tests/fixtures/codex_usage.json"));
        fetch_codex(&m, &cred(), now()).unwrap();
        let seen = m.seen.borrow();
        assert!(seen[0].1.iter().any(|(k, v)| k == "chatgpt-account-id" && v == "acct_1"));
    }

    #[test]
    fn http_errors_map() {
        let c = cred();
        assert!(matches!(
            fetch_claude(&Mock::new(401, ""), &c, now()),
            Err(FetchError::NeedsRelogin(_))
        ));
        assert!(matches!(
            fetch_claude(&Mock::new(429, ""), &c, now()),
            Err(FetchError::RateLimited { .. })
        ));
        assert!(matches!(
            fetch_codex(&Mock::new(500, ""), &c, now()),
            Err(FetchError::Network(_))
        ));
    }

    #[test]
    fn schema_change_becomes_parse_error_not_panic() {
        let m = Mock::new(200, r#"{"unexpected":true}"#);
        assert!(matches!(fetch_claude(&m, &cred(), now()), Err(FetchError::Parse(_))));
    }

    #[test]
    fn stale_fallback_keeps_previous_values_and_time() {
        let win = vec![Window { kind: WindowKind::Weekly, used_percent: 40.0, resets_at: None }];
        let prev_time = Utc.with_ymd_and_hms(2026, 10, 18, 6, 0, 0).unwrap();
        let prev = ServiceUsage { service: Service::Claude, windows: win.clone(), fetched_at: prev_time, status: Status::Ok };
        let r = resolve(Service::Claude, Some(&prev), Err(FetchError::RateLimited { retry_after_secs: None }), now());
        assert_eq!(r.windows, win);
        assert_eq!(r.fetched_at, prev_time);
        assert!(matches!(r.status, Status::Stale { .. }));
    }

    #[test]
    fn no_previous_means_unavailable() {
        let r = resolve(Service::Codex, None, Err(FetchError::Network("x".into())), now());
        assert!(r.windows.is_empty());
        assert!(matches!(r.status, Status::Unavailable { .. }));
    }

    #[test]
    fn error_text_never_contains_token() {
        let e = FetchError::Network("failed with Bearer tok-abcdefghijklmnopqrstuvwxyz".into());
        let r = resolve(Service::Claude, None, Err(e), now());
        match r.status {
            Status::Unavailable { reason } => assert!(!reason.contains("abcdefghijkl")),
            _ => panic!(),
        }
    }

    #[test]
    fn backoff() {
        assert_eq!(next_delay_secs(60, 0, None), 300); // 最小5分
        assert_eq!(next_delay_secs(300, 2, None), 1200);
        assert_eq!(next_delay_secs(300, 10, None), 3600); // 上限1時間
        assert_eq!(next_delay_secs(300, 0, Some(900)), 900);
        assert_eq!(next_delay_secs(300, 0, Some(99999)), 3600);
    }
}
