use std::fmt;

/// トークン等の秘匿値。Debug/Display は常に伏せ字。Serialize は実装しない
/// (誤ってログ・設定ファイル・IPCに出さないため)。
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(v: impl Into<String>) -> Self {
        Secret(v.into())
    }

    /// HTTP ヘッダ構築など、必要な箇所でのみ呼ぶ。
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

/// エラーメッセージやログに出す文字列から、トークンらしき長い文字列を伏せる。
/// 20文字以上の [A-Za-z0-9._-] 連続を対象にする。
pub fn redact(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if run.len() >= 20 {
            out.push_str("[REDACTED]");
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
            run.push(c);
        } else {
            flush(&mut run, &mut out);
            out.push(c);
        }
    }
    flush(&mut run, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_never_prints() {
        let s = Secret::new("sk-ant-oat01-abcdefghijklmnopqrstuvwxyz");
        assert!(!format!("{:?}", s).contains("sk-ant"));
        assert!(!format!("{}", s).contains("sk-ant"));
        assert_eq!(s.expose(), "sk-ant-oat01-abcdefghijklmnopqrstuvwxyz");
    }

    #[test]
    fn redact_masks_long_tokens_only() {
        let msg = "Bearer sk-ant-oat01-abcdefghijklmnopqrstuvwxyz failed: status 401";
        let r = redact(msg);
        assert!(!r.contains("abcdefghijkl"));
        assert!(r.contains("status 401"));
        assert!(r.contains("Bearer"));
    }
}
