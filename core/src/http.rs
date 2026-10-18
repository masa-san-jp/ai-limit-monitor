//! 本番用 HTTP 実装(feature = "reqwest-http")。
//! - https のみ / リダイレクト追従なし(トークンを別ホストへ送らない)/ タイムアウト15秒
//! - 許可リスト外のURLは送信前に拒否

#![cfg(feature = "reqwest-http")]

use std::time::Duration;

use crate::allowlist::is_allowed;
use crate::error::FetchError;
use crate::fetch::{HttpGet, HttpResponse};
use crate::secret::redact;

pub struct ReqwestHttp {
    client: reqwest::blocking::Client,
}

impl ReqwestHttp {
    pub fn new() -> Result<Self, FetchError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .user_agent("ai-limit-monitor/0.1")
            .build()
            .map_err(|e| FetchError::Network(redact(&e.to_string())))?;
        Ok(Self { client })
    }
}

impl HttpGet for ReqwestHttp {
    fn get(&self, url: &str, headers: &[(String, String)]) -> Result<HttpResponse, FetchError> {
        if !is_allowed(url) {
            return Err(FetchError::NotAllowed);
        }
        let mut req = self.client.get(url);
        for (k, v) in headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let res = req
            .send()
            .map_err(|e| FetchError::Network(redact(&e.without_url().to_string())))?;
        let status = res.status().as_u16();
        let retry_after_secs = res
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok());
        let body = res
            .text()
            .map_err(|e| FetchError::Network(redact(&e.without_url().to_string())))?;
        Ok(HttpResponse { status, body, retry_after_secs })
    }
}
