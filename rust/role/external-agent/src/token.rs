//! Mint a short-TTL, run-scoped token for an external-agent run (agent-key-lifecycle D1–D5). The
//! token carries the derived principal's caps + the caller's constraint + the `run_id`, signed
//! with the node key so the gateway verifies it on `POST /mcp/call` exactly as it does for a UI
//! session token. The shim holds this token as its only credential.
//!
//! **Why the constraint is in the token.** A verified token produces a `Principal` via
//! `lb_auth::verify`, which (unlike `Principal::derive`) does NOT re-introduce the caller bound —
//! gate 2b (`caps::check`'s delegation gate) only fires when `Principal.constraint` is `Some`. So
//! the caller's caps MUST ride IN the token as the `constraint` claim; `verify` copies it onto
//! the principal, and gate 2b enforces `agent ∩ caller` per call. Without this, a run token would
//! widen to the agent's own caps (losing the human's bound) — the asymmetry the constraint claim
//! closes. The mint is the ONE place this is set outside `Principal::derive`.
//!
//! **TTL (D1):** 5 minutes, configurable per `AgentProfile`. The TTL is the *soft*-revoke bound
//! (hard cancel is instant via D3 — the gateway's run-status gate); we don't shorten it to chase
//! hard-stop latency (that just adds churn, per D1). **Refresh lead (D2):** 60% of TTL — a
//! fraction so it scales if the TTL is reconfigured. The shim refreshes at this timestamp before
//! the next call AND self-heals on a 401 (one-shot).

use lb_auth::{mint, Claims, Principal, Role, SigningKey};

/// The default time-to-live for a run token (D1: 5 minutes). The soft-revoke ceiling; hard
/// cancel is instant via the run-status gate (D3).
pub const DEFAULT_RUN_TOKEN_TTL_SECS: u64 = 5 * 60;

/// The mint result — the bearer the shim holds plus the unix-second timestamp at which the shim
/// should proactively refresh (60% of TTL, D2).
#[derive(Debug, Clone)]
pub struct RunToken {
    pub token: String,
    pub refresh_at_sec: u64,
    /// The expiry stamped into the token (the shim does not read it; the gateway's `verify` does).
    pub exp_sec: u64,
}

/// Mint a fresh run-scoped token for `run_id` under `key`, carrying `principal`'s caps +
/// constraint. `now` is the caller-injected logical clock (testing §3 — never wall-clock). `ttl`
/// is the token's lifetime in seconds (D1); the refresh lead is 60% of it (D2).
///
/// The principal is the **derived** one (`caller.derive("agent:session", agent_caps)`) so its
/// `caps` are the agent's and its `constraint` is the caller's. Both ride in the token so the
/// verified principal on the gateway matches what `caps::check` would have seen in-process.
pub fn mint_run_token(
    key: &SigningKey,
    principal: &Principal,
    run_id: &str,
    now: u64,
    ttl: u64,
) -> RunToken {
    let exp_sec = now.saturating_add(ttl);
    let refresh_at_sec = now.saturating_add(ttl * 3 / 5);
    let claims = Claims {
        sub: principal.sub().to_string(),
        ws: principal.ws().to_string(),
        role: Role::Member, // a run-scoped actor is never more privileged than a member
        caps: principal.caps().to_vec(),
        iat: now,
        exp: exp_sec,
        constraint: principal.constraint().map(|c| c.to_vec()),
        run_id: Some(run_id.to_string()),
    };
    RunToken {
        token: mint(key, &claims),
        refresh_at_sec,
        exp_sec,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_auth::verify;

    fn derived(caps: &[&str], constraint: &[&str]) -> Principal {
        let caller = Principal::routed(
            "user:ada",
            "ws-a",
            constraint.iter().map(|s| s.to_string()).collect(),
        );
        caller.derive(
            "agent:session",
            caps.iter().map(|s| s.to_string()).collect(),
        )
    }

    #[test]
    fn token_carries_run_id_and_constraint() {
        let key = SigningKey::generate();
        let p = derived(
            &["mcp:tools.catalog:call", "mcp:devkit.scaffold:call"],
            &["mcp:tools.catalog:call"],
        );
        let rt = mint_run_token(&key, &p, "job-1", 1000, DEFAULT_RUN_TOKEN_TTL_SECS);
        let v = verify(&key, &rt.token, 1001).expect("verifies");
        assert_eq!(v.run_id(), Some("job-1"));
        // The constraint is preserved — gate 2b will fire on the verified principal.
        assert!(v.constraint().is_some());
        assert_eq!(v.ws(), "ws-a");
    }

    #[test]
    fn refresh_lead_is_60_percent_of_ttl() {
        let key = SigningKey::generate();
        let p = derived(&["mcp:x:call"], &["mcp:x:call"]);
        let now = 1_000_000;
        let ttl = 300_u64;
        let rt = mint_run_token(&key, &p, "r", now, ttl);
        assert_eq!(rt.exp_sec, now + ttl);
        // 60% of 300 = 180 → refresh at now + 180.
        assert_eq!(rt.refresh_at_sec, now + 180);
    }

    #[test]
    fn token_without_constraint_still_works() {
        // A non-derived principal (routed) has no constraint — the token carries None and the
        // verified principal is bounded by caps alone (the ordinary path).
        let key = SigningKey::generate();
        let p = Principal::routed("user:ada", "ws-a", vec!["mcp:x:call".into()]);
        let rt = mint_run_token(&key, &p, "r", 0, DEFAULT_RUN_TOKEN_TTL_SECS);
        let v = verify(&key, &rt.token, 1).expect("verifies");
        assert_eq!(v.run_id(), Some("r"));
        assert!(v.constraint().is_none());
    }
}
