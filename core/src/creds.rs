//! 既存CLIの認証情報を「読み取り専用」で参照する。
//! - アプリ側にコピーを保存しない / 更新(リフレッシュ)しない / ログに出さない
//! - 期限切れは NeedsRelogin として扱い、CLIでの再ログインを案内する

use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;

use crate::error::FetchError;
use crate::secret::Secret;

pub struct Credential {
    pub token: Secret,
    /// Codex の chatgpt-account-id ヘッダ用(任意)
    pub account_id: Option<String>,
}

// ---------- Claude ----------
#[derive(Deserialize)]
struct ClaudeFile {
    #[serde(rename = "claudeAiOauth")]
    oauth: Option<ClaudeOauth>,
}

#[derive(Deserialize)]
struct ClaudeOauth {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    /// ミリ秒エポック
    #[serde(rename = "expiresAt")]
    expires_at: Option<i64>,
}

pub fn claude_from_json(json: &str, now: DateTime<Utc>) -> Result<Credential, FetchError> {
    let f: ClaudeFile = serde_json::from_str(json)
        .map_err(|_| FetchError::NeedsRelogin("Claude Code の認証情報を読めません".into()))?;
    let o = f
        .oauth
        .ok_or_else(|| FetchError::NeedsRelogin("Claude Code にログインしてください".into()))?;
    let token = o
        .access_token
        .filter(|t| !t.is_empty())
        .ok_or_else(|| FetchError::NeedsRelogin("Claude Code にログインしてください".into()))?;
    if let Some(ms) = o.expires_at {
        if let Some(exp) = Utc.timestamp_millis_opt(ms).single() {
            if exp <= now {
                return Err(FetchError::NeedsRelogin(
                    "Claude のトークン期限切れ。`claude` を起動して再認証してください".into(),
                ));
            }
        }
    }
    Ok(Credential { token: Secret::new(token), account_id: None })
}

// ---------- Codex ----------
#[derive(Deserialize)]
struct CodexFile {
    tokens: Option<CodexTokens>,
}

#[derive(Deserialize)]
struct CodexTokens {
    access_token: Option<String>,
    account_id: Option<String>,
}

pub fn codex_from_json(json: &str) -> Result<Credential, FetchError> {
    let f: CodexFile = serde_json::from_str(json)
        .map_err(|_| FetchError::NeedsRelogin("Codex の認証情報を読めません".into()))?;
    let t = f
        .tokens
        .ok_or_else(|| FetchError::NeedsRelogin("Codex CLI にログインしてください".into()))?;
    let token = t
        .access_token
        .filter(|s| !s.is_empty())
        .ok_or_else(|| FetchError::NeedsRelogin("Codex CLI にログインしてください".into()))?;
    Ok(Credential { token: Secret::new(token), account_id: t.account_id })
}

// ---------- 実ストアからの読み出し(macOS) ----------

/// macOS Keychain の汎用パスワードを読む(`security` コマンド経由、読み取りのみ)。
/// 初回は macOS がアクセス許可ダイアログを出す。
#[cfg(target_os = "macos")]
pub fn read_keychain(service: &str) -> Result<String, FetchError> {
    let out = std::process::Command::new("security")
        .args(["find-generic-password", "-s", service, "-w"])
        .output()
        .map_err(|e| FetchError::Network(format!("security 実行失敗: {e}")))?;
    if !out.status.success() {
        return Err(FetchError::NeedsRelogin(format!("Keychain に {service} がありません")));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn read_keychain(_service: &str) -> Result<String, FetchError> {
    Err(FetchError::NeedsRelogin("Keychain は macOS のみ対応".into()))
}

fn home_file(rel: &str) -> Result<String, FetchError> {
    let home = std::env::var("HOME").map_err(|_| FetchError::NeedsRelogin("HOME が未設定".into()))?;
    std::fs::read_to_string(format!("{home}/{rel}"))
        .map_err(|_| FetchError::NeedsRelogin(format!("~/{rel} がありません")))
}

pub fn load_claude(now: DateTime<Utc>) -> Result<Credential, FetchError> {
    let json = read_keychain("Claude Code-credentials").or_else(|_| home_file(".claude/.credentials.json"))?;
    claude_from_json(&json, now)
}

pub fn load_codex() -> Result<Credential, FetchError> {
    codex_from_json(&home_file(".codex/auth.json")?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 18, 6, 31, 0).unwrap()
    }

    #[test]
    fn claude_ok() {
        let exp = (now().timestamp() + 3600) * 1000;
        let j = format!(r#"{{"claudeAiOauth":{{"accessToken":"tok-abcdefghijklmnopqrstuvwxyz","expiresAt":{exp}}}}}"#);
        let c = claude_from_json(&j, now()).unwrap();
        assert_eq!(c.token.expose(), "tok-abcdefghijklmnopqrstuvwxyz");
    }

    #[test]
    fn claude_expired_needs_relogin_without_leaking_token() {
        let exp = (now().timestamp() - 10) * 1000;
        let j = format!(r#"{{"claudeAiOauth":{{"accessToken":"tok-abcdefghijklmnopqrstuvwxyz","expiresAt":{exp}}}}}"#);
        let e = match claude_from_json(&j, now()) {
            Err(e) => e,
            Ok(_) => panic!("expected error"),
        };
        assert!(matches!(e, FetchError::NeedsRelogin(_)));
        assert!(!e.to_string().contains("tok-abcdef"));
    }

    #[test]
    fn claude_missing() {
        assert!(matches!(claude_from_json("{}", now()), Err(FetchError::NeedsRelogin(_))));
        assert!(matches!(claude_from_json("garbage", now()), Err(FetchError::NeedsRelogin(_))));
    }

    #[test]
    fn codex_ok_and_missing() {
        let c = codex_from_json(r#"{"tokens":{"access_token":"abc","account_id":"acct_1"}}"#).unwrap();
        assert_eq!(c.account_id.as_deref(), Some("acct_1"));
        assert!(matches!(codex_from_json("{}"), Err(FetchError::NeedsRelogin(_))));
    }
}
