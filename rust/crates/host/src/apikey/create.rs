//! `apikey.create` — mint a workspace-walled API key (api-keys scope). Gated by
//! `mcp:apikey.manage:call`, workspace-first. Generates a fresh id + high-entropy secret, stores
//! ONLY the peppered hash, assigns the chosen role (+ any narrowing caps) to the key's
//! `Subject::Key` grant, and returns the one-time bearer string `lbk_{ws}.{id}.{secret}` — the
//! ONLY time the raw secret leaves the host.
//!
//! **The privilege-escalation guard (load-bearing).** The grant path's `grants_assign` EXEMPTS
//! `role:` grants from no-widening (a role's caps were bounded at define time). But a key assigned
//! the system-seeded `apikey-write` role would otherwise inherit caps the *creator* does not hold.
//! So `apikey.create` computes the key's EFFECTIVE resolved caps (the role's caps ∪ the extra caps)
//! and refuses creation unless that set ⊆ the creator's own caps — covering BOTH the custom-cap and
//! the built-in-role paths the scope flags. This is the case `grants_assign` does NOT cover.

use std::collections::BTreeSet;

use lb_apikey::{display_prefix, generate_id, generate_secret};
use lb_auth::Principal;
use lb_authz::{grant_assign, role_caps};
use lb_mcp::authorize_tool;
use lb_outbox::Effect;
use lb_store::{write, Store};

use super::error::ApiKeyError;
use super::model::{key_subject, ApiKeyRecord, TABLE};
use super::seed::ensure_builtin_roles;

/// The outbox target/action for an expiry housekeeping effect (best-effort; security is the
/// auth-time lazy check, never this effect firing).
const EXPIRY_TARGET: &str = "apikey";
const EXPIRY_ACTION: &str = "expire";

/// Create a key in `ws` as `principal`, returning the one-time bearer string.
///
/// - `role`: a role name to assign (`apikey-read` / `apikey-write` / a custom role, or empty for
///   caps-only). The role's caps are folded into the effective set and no-widening-checked.
/// - `extra_caps`: additional narrowing caps granted directly to the key (also no-widening-checked).
/// - `expires_at`: unix-secs expiry (`0` = never). Enforced lazily at auth; an outbox effect is
///   enqueued for best-effort housekeeping.
/// - `now`: the caller-injected logical clock (testing §3).
#[allow(clippy::too_many_arguments)]
pub async fn apikey_create(
    store: &Store,
    principal: &Principal,
    ws: &str,
    pepper: &[u8],
    label: &str,
    kind: &str,
    role: &str,
    extra_caps: &[String],
    expires_at: u64,
    now: u64,
) -> Result<String, ApiKeyError> {
    authorize_tool(principal, ws, "apikey.manage").map_err(|_| ApiKeyError::Denied)?;
    if label.is_empty() {
        return Err(ApiKeyError::BadInput("label is required".into()));
    }

    // Ensure the built-in role rows exist so role_caps resolves them (idempotent).
    ensure_builtin_roles(store, ws).await?;

    // The key's effective caps = the chosen role's caps ∪ the extra caps. No-widening: every one
    // must be a cap the CREATOR holds (the privilege-escalation guard, incl. the role path that
    // grants_assign would otherwise exempt).
    let role_caps_vec = if role.is_empty() {
        Vec::new()
    } else {
        role_caps(store, ws, role).await?
    };
    let mut effective: BTreeSet<String> = role_caps_vec.into_iter().collect();
    for cap in extra_caps {
        effective.insert(cap.clone());
    }
    for cap in &effective {
        if !crate::authz::holds_cap(principal, ws, cap) {
            return Err(ApiKeyError::Widen(cap.clone()));
        }
    }

    // Generate the credential. The secret leaves ONLY in the returned bearer string below.
    let id = generate_id();
    let secret = generate_secret();
    let key_hash = lb_apikey::key_hash(pepper, &secret);
    let prefix = display_prefix(ws, &id);

    let record = ApiKeyRecord::new(&id, ws, label, kind, &key_hash, &prefix, now, expires_at);
    let value =
        serde_json::to_value(&record).map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;

    // Persist the record. If it expires, enqueue the housekeeping effect transactionally WITH the
    // record (the outbox's next_attempt_ts is the future-scheduler; the auth-time check is the
    // security floor, never this effect firing).
    if expires_at != 0 {
        let effect = expiry_effect(&id, ws, expires_at, now);
        lb_outbox::enqueue(store, ws, TABLE, &id, &value, &effect).await?;
    } else {
        write(store, ws, TABLE, &id, &value).await?;
    }

    // Assign the role + extra caps to the key's subject (raw grant_assign — this verb IS the
    // chokepoint, and the no-widening check above already ran).
    let subject = key_subject(&id);
    if !role.is_empty() {
        grant_assign(store, ws, &subject, &format!("role:{role}")).await?;
    }
    for cap in extra_caps {
        grant_assign(store, ws, &subject, cap).await?;
    }

    Ok(lb_apikey::format_bearer(ws, &id, &secret))
}

/// Build the best-effort expiry housekeeping effect for a key: due at `expires_at`, idempotent on
/// the key id. A relay/tick that processes it tombstones the record; if it never fires, the
/// auth-time `now >= expires_at` check still refuses the key.
fn expiry_effect(id: &str, ws: &str, expires_at: u64, now: u64) -> Effect {
    let mut effect = Effect::new(
        format!("apikey-expire-{id}"),
        EXPIRY_TARGET,
        EXPIRY_ACTION,
        format!("{{\"id\":\"{id}\",\"ws\":\"{ws}\"}}"),
        format!("apikey:expire:{id}"),
        now,
    );
    effect.next_attempt_ts = expires_at;
    effect
}
