//! The **select-token** — the deliberately powerless JWT the N-workspace branch of `/auth/login`
//! returns (email-login scope). It is good for exactly ONE thing: `POST /auth/select {workspace}` →
//! the full token. It is NOT a session; it carries the person's `sub` so `/auth/select` knows whom to
//! mint for, but `ws: ""`, `caps: []`, and a `constraint: ["ws-select"]` marker so every normal gate
//! refuses it (empty ws fails the wall, empty caps fail the capability gate) and only `/auth/select`
//! accepts it, by checking the marker explicitly.
//!
//! Reusing the existing `constraint` claim (the delegation bound) is deliberate: a select-token's
//! `constraint` is the single sentinel `["ws-select"]`, never a real caps list, so `caps::check`'s
//! delegation intersection (`caps ∩ constraint`) can never pass — `caps` is already empty. One token
//! type, one signer, one new acceptor; no parallel session store (the scope's rejected-alternatives).

use lb_auth::{mint, Claims, Principal, Role, SigningKey};

/// The `constraint` sentinel that marks a token as a select-token (and NOTHING else). A real
/// delegated principal carries a caps list here; a select-token carries only this string, which is
/// not a capability, so it authorizes nothing and merely tags the token for `/auth/select`.
pub const WS_SELECT_CONSTRAINT: &str = "ws-select";

/// The select-token lifetime — long enough to pick a workspace, short enough that a walked-away user
/// returns to a dead token (the rubix-ai picker fails soft to the login form). ~5 minutes.
pub const SELECT_TTL_SECS: u64 = 5 * 60;

/// Mint a select-token for `sub`: `ws:""`, `caps:[]`, `constraint:["ws-select"]`, ~5-min TTL. Signed
/// with the node key like any token, but powerless everywhere except `/auth/select`.
pub fn mint_select_token(key: &SigningKey, sub: &str, now: u64) -> String {
    let claims = Claims {
        sub: sub.to_string(),
        ws: String::new(),
        role: Role::Member,
        caps: Vec::new(),
        iat: now,
        exp: now.saturating_add(SELECT_TTL_SECS),
        constraint: Some(vec![WS_SELECT_CONSTRAINT.to_string()]),
        run_id: None,
    };
    mint(key, &claims)
}

/// Is `principal` a verified select-token? True iff its `constraint` is exactly the `ws-select`
/// sentinel. `/auth/select` calls this to positively accept a select-token (and only there); every
/// other route/verb refuses it structurally (empty ws + empty caps), so this is the one acceptor.
pub fn is_select_token(principal: &Principal) -> bool {
    matches!(principal.constraint(), Some([only]) if only == WS_SELECT_CONSTRAINT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_auth::verify;

    #[test]
    fn a_select_token_is_powerless_and_recognized() {
        let key = SigningKey::generate();
        let now = 1000;
        let token = mint_select_token(&key, "user:bob", now);
        let principal = verify(&key, &token, now).expect("select-token verifies");
        // Powerless: no workspace, no caps — every normal gate fails on it.
        assert_eq!(principal.ws(), "", "select-token carries no workspace");
        assert!(principal.caps().is_empty(), "select-token carries no caps");
        // But it IS recognized by its one acceptor.
        assert!(is_select_token(&principal), "the ws-select marker is set");
        assert_eq!(principal.sub(), "user:bob", "it names whom to mint for");
    }

    #[test]
    fn a_select_token_expires() {
        let key = SigningKey::generate();
        let token = mint_select_token(&key, "user:bob", 1000);
        // Past the ~5-min TTL → expired.
        assert!(verify(&key, &token, 1000 + SELECT_TTL_SECS + 1).is_err());
    }

    #[test]
    fn an_ordinary_token_is_not_a_select_token() {
        use lb_auth::{mint, Claims, Role};
        let key = SigningKey::generate();
        let claims = Claims {
            sub: "user:ada".into(),
            ws: "acme".into(),
            role: Role::Member,
            caps: vec!["mcp:dashboard.list:call".into()],
            iat: 1000,
            exp: 100000,
            constraint: None,
            run_id: None,
        };
        let token = mint(&key, &claims);
        let principal = verify(&key, &token, 1000).unwrap();
        assert!(
            !is_select_token(&principal),
            "a full token is not a select-token"
        );
    }
}
