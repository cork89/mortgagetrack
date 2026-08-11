//! Safe post-login redirect targets.

/// Allow only same-origin relative paths (no scheme/host open redirects).
pub fn safe_next(next: Option<&str>) -> Option<&str> {
    let next = next?.trim();
    if next.is_empty() {
        return None;
    }
    if !next.starts_with('/') || next.starts_with("//") || next.contains('\\') {
        return None;
    }
    if next.contains('\n') || next.contains('\r') {
        return None;
    }
    Some(next)
}

/// Extract a share invite token from a `/share/{token}` next path.
pub fn share_token_from_next(next: &str) -> Option<&str> {
    let path = safe_next(Some(next))?;
    let token = path.strip_prefix("/share/")?;
    if token.is_empty() || token.len() > 128 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(token)
}

/// Percent-encode a value for use in a query string.
pub fn encode_query_value(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0xf) as usize] as char);
            }
        }
    }
    out
}

pub fn is_share_invite_next(next: &str) -> bool {
    share_token_from_next(next).is_some()
}

#[cfg(test)]
mod tests {
    use super::{encode_query_value, share_token_from_next};

    #[test]
    fn encodes_share_path_for_query() {
        assert_eq!(encode_query_value("/share/abc123"), "%2Fshare%2Fabc123");
    }

    #[test]
    fn parses_share_token_from_next() {
        assert_eq!(share_token_from_next("/share/deadbeef"), Some("deadbeef"));
        assert_eq!(share_token_from_next("/evil//share/x"), None);
    }
}
