use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum FetchError {
    /// トークン失効・未ログイン。CLIでの再ログインが必要(アプリは更新しない)
    #[error("再ログインが必要です: {0}")]
    NeedsRelogin(String),
    /// 429。次回まで待つ
    #[error("レート制限(429)")]
    RateLimited { retry_after_secs: Option<u64> },
    #[error("ネットワークエラー: {0}")]
    Network(String),
    #[error("レスポンス解析失敗: {0}")]
    Parse(String),
    #[error("許可されていない通信先です")]
    NotAllowed,
}
