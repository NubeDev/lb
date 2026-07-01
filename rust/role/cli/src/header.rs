//! The `ws: … user: … role: …` header every command prints (operator-cli scope, Goals: "output that
//! makes the wall legible" — PRODUCT.md principle #1). Scope is never ambiguous: before any result the
//! CLI states which workspace, which user, and which role the call ran under.
//!
//! The header's source of truth is the **session's claims** — remote decodes them from the token it
//! holds (`lb_auth::claims_unverified`); local reads them from the minted principal. Both collapse to
//! this one [`Header`] so the render (and the "never leak the token" discipline) lives in one place,
//! unit-tested, not scattered in `main.rs`. The token itself is NEVER part of the header.

use std::fmt;

use lb_auth::Role;

/// The identity facts a command header shows. Built by a transport from its session (a decoded token
/// remotely, the minted claims locally) — deliberately just the three the operator needs to read the
/// wall, plus the mode so local vs remote is visible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub workspace: String,
    pub user: String,
    pub role: Role,
    /// `true` when the call ran through the in-process local node (`lb local` / `--local`), `false`
    /// for the remote gateway. Rendered so an operator never confuses an offline local run with a
    /// remote one.
    pub local: bool,
}

impl Header {
    /// Build a header from raw parts (a transport supplies these from its session).
    pub fn new(
        workspace: impl Into<String>,
        user: impl Into<String>,
        role: Role,
        local: bool,
    ) -> Self {
        Self {
            workspace: workspace.into(),
            user: user.into(),
            role,
            local,
        }
    }

    /// The kebab-case role label (`member`, `workspace-admin`, `super-admin`) — matches the token's
    /// serde rename so the header reads the same string the claim carries.
    pub fn role_label(&self) -> &'static str {
        match self.role {
            Role::SuperAdmin => "super-admin",
            Role::WorkspaceAdmin => "workspace-admin",
            Role::Member => "member",
        }
    }

    /// The one-line header string. `mode:` is included so `lb local …` is visibly offline.
    pub fn render(&self) -> String {
        let mode = if self.local { "local" } else { "remote" };
        format!(
            "ws: {}  user: {}  role: {}  mode: {}",
            self.workspace,
            self.user,
            self.role_label(),
            mode
        )
    }
}

impl fmt::Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

/// Build a header from a session token by decoding its (unverified) claims — the remote transport's
/// source. `None` if the token payload is unreadable. The signature is NOT checked here (the server
/// verifies every request); this only labels the caller's own terminal.
pub fn header_from_token(token: &str, local: bool) -> Option<Header> {
    let claims = lb_auth::claims_unverified(token)?;
    Some(Header::new(claims.ws, claims.sub, claims.role, local))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_auth::{mint, Claims, SigningKey};

    fn sample_token(ws: &str, sub: &str, role: Role) -> String {
        let key = SigningKey::generate();
        let claims = Claims {
            sub: sub.into(),
            ws: ws.into(),
            role,
            caps: vec!["mcp:inbox.list:call".into()],
            iat: 0,
            exp: u64::MAX,
        };
        mint(&key, &claims)
    }

    #[test]
    fn renders_ws_user_role_mode() {
        let h = Header::new("acme", "user:ada", Role::Member, false);
        assert_eq!(
            h.render(),
            "ws: acme  user: user:ada  role: member  mode: remote"
        );
    }

    #[test]
    fn local_mode_is_visible_in_the_header() {
        let h = Header::new("acme", "user:ada", Role::Member, true);
        assert!(h.render().contains("mode: local"));
    }

    #[test]
    fn role_labels_are_kebab_case() {
        assert_eq!(
            Header::new("w", "u", Role::WorkspaceAdmin, false).role_label(),
            "workspace-admin"
        );
        assert_eq!(
            Header::new("w", "u", Role::SuperAdmin, false).role_label(),
            "super-admin"
        );
        assert_eq!(
            Header::new("w", "u", Role::Member, false).role_label(),
            "member"
        );
    }

    #[test]
    fn header_from_token_decodes_the_claims() {
        let tok = sample_token("acme", "user:ada", Role::Member);
        let h = header_from_token(&tok, false).expect("token decodes");
        assert_eq!(h.workspace, "acme");
        assert_eq!(h.user, "user:ada");
        assert_eq!(h.role, Role::Member);
    }

    #[test]
    fn the_header_never_contains_the_token() {
        // The token-custody discipline: the header renders identity, never the secret it was decoded
        // from. A regression here would leak the bearer into every command's first line.
        let tok = sample_token("acme", "user:ada", Role::Member);
        let h = header_from_token(&tok, false).unwrap();
        assert!(
            !h.render().contains(&tok),
            "the header must never echo the token"
        );
    }
}
