//! A best-effort **secret lint** for `agent.memory.set` (agent-memory scope: "the `set` verb should
//! reject obvious secret shapes (best-effort lint, not a gate)"). Memory is workspace-authored fact
//! text, never a credential store — a fact that looks like `AKIA…`/`sk-…`/a PEM block/`password: …`
//! is almost certainly a leaked secret the agent should not persist. This catches the obvious shapes
//! and returns a clear rejection so a poisoned/careless write is stopped early.
//!
//! It is a **lint, not a security boundary**: the real protection is that memory never widens
//! authority (the wall). A determined encoder gets past a regex — that is fine; the point is to stop
//! the accidental `password: hunter2` and the pasted API key, not to be a DLP engine.

/// Return `Some(reason)` if `text` (the concatenated description + body) contains an obvious secret
/// shape; `None` if it looks clean. Case-insensitive on the keyword shapes.
pub fn looks_like_secret(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();

    // A PEM private-key block — unambiguous.
    if text.contains("-----BEGIN") && lower.contains("private key") {
        return Some("looks like a private key (PEM block)");
    }
    // AWS access key id.
    if contains_token_prefixed(text, "AKIA", 16) {
        return Some("looks like an AWS access key id (AKIA…)");
    }
    // OpenAI-style / generic `sk-` secret.
    if contains_token_prefixed(text, "sk-", 20) {
        return Some("looks like an API secret (sk-…)");
    }
    // GitHub token families.
    for p in ["ghp_", "gho_", "ghs_", "github_pat_"] {
        if contains_token_prefixed(text, p, p.len() + 20) {
            return Some("looks like a GitHub token");
        }
    }
    // A `password:`/`secret:`/`api_key:`/`token:` assignment with a non-trivial value.
    for kw in ["password", "passwd", "secret", "api_key", "apikey", "token"] {
        if let Some(pos) = lower.find(kw) {
            let after = &lower[pos + kw.len()..];
            let after = after.trim_start();
            if let Some(rest) = after.strip_prefix([':', '=']) {
                let val = rest.trim().trim_matches(['"', '\'']);
                // A real assigned value (not an empty placeholder or a prose mention).
                if val.len() >= 6 && !val.starts_with('<') && !val.contains(' ') {
                    return Some("looks like an assigned credential (password/secret/token: …)");
                }
            }
        }
    }
    None
}

/// True if `text` contains `prefix` followed by at least enough non-space token characters to reach a
/// total token length of `min_total` — i.e. a real key, not a bare mention of the prefix in prose.
fn contains_token_prefixed(text: &str, prefix: &str, min_total: usize) -> bool {
    let mut search = text;
    while let Some(pos) = search.find(prefix) {
        let after = &search[pos + prefix.len()..];
        let token_tail: usize = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == '/')
            .count();
        if prefix.len() + token_tail >= min_total {
            return true;
        }
        // Advance past this occurrence and keep looking.
        search = &search[pos + prefix.len()..];
    }
    false
}
