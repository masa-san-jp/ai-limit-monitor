/// 通信先の許可リスト。https かつ完全一致ホストのみ許可。
const ALLOWED_HOSTS: &[&str] = &["api.anthropic.com", "chatgpt.com", "grok.com"];

pub fn is_allowed(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    // userinfo(user@host)やポート指定は拒否
    if authority.contains('@') || authority.contains(':') {
        return false;
    }
    ALLOWED_HOSTS.contains(&authority.to_ascii_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_known_hosts() {
        assert!(is_allowed("https://api.anthropic.com/api/oauth/usage"));
        assert!(is_allowed("https://chatgpt.com/backend-api/wham/usage"));
        assert!(is_allowed("https://grok.com/"));
    }

    #[test]
    fn rejects_others() {
        assert!(!is_allowed("http://api.anthropic.com/x"));
        assert!(!is_allowed("https://evil.example.com/"));
        assert!(!is_allowed("https://api.anthropic.com.evil.com/"));
        assert!(!is_allowed("https://api.anthropic.com@evil.com/"));
        assert!(!is_allowed("https://chatgpt.com:8443/"));
        assert!(!is_allowed("https://notchatgpt.com/"));
    }
}
