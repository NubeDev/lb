//! Workspace-scope a bus key. Callers pass a workspace-relative key (`chan/general`); this
//! prepends the `ws/{id}/` prefix that makes the workspace wall structural on the bus.
//!
//! The matching capability (`bus:chan/*:sub`) is written WITHOUT the prefix — the host adds
//! it on both the cap-check and the publish, so the two always agree (auth-caps scope).

/// Build the full bus key for `rel` within workspace `ws`: `ws/{ws}/{rel}`.
pub fn ws_key(ws: &str, rel: &str) -> String {
    let rel = rel.trim_start_matches('/');
    format!("ws/{ws}/{rel}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_workspace() {
        assert_eq!(ws_key("acme", "chan/general"), "ws/acme/chan/general");
    }

    #[test]
    fn tolerates_leading_slash() {
        assert_eq!(ws_key("acme", "/chan/general"), "ws/acme/chan/general");
    }
}
