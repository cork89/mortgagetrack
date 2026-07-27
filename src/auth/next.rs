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
