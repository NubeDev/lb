//! The **global identity directory** — one record per person, in a reserved **system namespace**
//! `_lb_identity` (global-identity scope, decision #1). NOT a workspace: identity is the one thing
//! lifted out of the per-workspace wall, hub-writable and resolution-read-only, carrying no tenant
//! data — only `{sub, display_name?, created_ts}`. Mirrors the shipped reserved-namespace pattern
//! (`_lb_workflow_directory` / `_lb_workspaces`): the leading underscore marks it system-internal,
//! disallowed as a real workspace by operator convention.
//!
//! `sub` is the human handle (`user:ada`), **globally unique** (decision #6 — keeping it avoids
//! retrofitting every existing `Subject::User(sub)` grant row). `display_name` is a separate,
//! non-unique, per-identity field. No credential in v1 (decision #7) — the dev-login sits behind the
//! identity-resolution seam; OIDC attaches additively later.
//!
//! Raw verbs, no authorization here — the host `identity` service is the capability chokepoint
//! (`mcp:identity.manage:call`). Hub-only writes (decision #8); edges verify tokens offline and read
//! cached identity only. One verb per file would over-split a 4-verb record family; this is the raw
//! store layer (the host service is split per verb).

use lb_store::{list as store_list, read, write, Store, StoreError};
use serde::{Deserialize, Serialize};

/// The reserved system namespace the identity directory lives in. Leading underscore marks it
/// system-internal; operators must not name a real workspace this (the same convention as
/// `_lb_workflow_directory` / `_lb_workspaces`).
pub const IDENTITY_NS: &str = "_lb_identity";

/// The table within that namespace.
pub const IDENTITY_TABLE: &str = "identity";

/// The constant `kind` discriminant so [`identity_list`] can equality-filter every row.
pub const IDENTITY_KIND: &str = "identity";

/// The reverse-index table mapping a lower-cased email → the `sub` that owns it (email-login scope).
/// A separate record family (not a field-scan) so uniqueness is a race-safe first-write via
/// [`store create`](lb_store::create): the index id IS the folded email, so two identities claiming
/// the same email collide on `CREATE` — `StoreError::Conflict`, never a silent overwrite. Lives in the
/// same reserved `_lb_identity` namespace; carries only the owning `sub` (no secret).
pub const IDENTITY_EMAIL_TABLE: &str = "identity_email";

/// A global identity: the globally-unique `sub` (the grant key, `user:ada`), an optional non-unique
/// display name, an optional globally-unique **email** (the human login handle, email-login scope),
/// and the created timestamp. Still secret-free — the credential is a SEPARATE record
/// (`identity_credential`), never a field here (§6.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    /// The globally-unique principal id (`user:ada`) — the same across workspaces; grants key on it.
    pub sub: String,
    /// A non-unique, per-identity human display name. Separate from `sub` (decision #6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// The human login handle (email-login scope) — globally unique, stored **lower-cased** so lookup
    /// is case-insensitive. Optional so a machine/agent identity (or a pre-email identity) has none.
    /// The uniqueness is enforced by the `identity_email` reverse index, not by this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Constant discriminant so `identity_list` selects every row.
    pub kind: String,
    /// Caller-injected logical timestamp (no wall-clock — testing §3).
    pub created_ts: u64,
}

impl Identity {
    pub fn new(sub: impl Into<String>, display_name: Option<String>, created_ts: u64) -> Self {
        Self {
            sub: sub.into(),
            display_name,
            email: None,
            kind: IDENTITY_KIND.to_string(),
            created_ts,
        }
    }
}

/// Fold an email to its canonical lookup form: trimmed + lower-cased. The one place case/whitespace
/// normalization happens, so a set and a later lookup agree (`Ada@ACME.com ` ≡ `ada@acme.com`).
pub fn fold_email(email: &str) -> String {
    email.trim().to_lowercase()
}

/// Create (or upsert) global identity `sub`. Idempotent on `sub` (re-creating updates the display
/// name / created_ts — last write wins). Provisions in NO workspace (decision #4 — joining is
/// `membership.add`).
pub async fn identity_create(
    store: &Store,
    sub: &str,
    display_name: Option<&str>,
    created_ts: u64,
) -> Result<Identity, StoreError> {
    let mut identity = Identity::new(sub, display_name.map(|s| s.to_string()), created_ts);
    // Preserve an existing email across an idempotent re-create (upsert must not silently drop it —
    // `create` re-provisions the same identity and would otherwise blank the login handle).
    if let Some(existing) = identity_get(store, sub).await? {
        identity.email = existing.email;
    }
    let value = serde_json::to_value(&identity).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, IDENTITY_NS, IDENTITY_TABLE, sub, &value).await?;
    Ok(identity)
}

/// Create global identity `sub` WITH an email in one race-safe step (email-login scope) — the
/// provisioning path (`identity.create {sub, email}`). Claims the reverse index first (so a duplicate
/// email fails before the identity record is touched), then writes the record. `Conflict` iff the
/// email is owned by a different identity.
pub async fn identity_create_with_email(
    store: &Store,
    sub: &str,
    display_name: Option<&str>,
    email: &str,
    created_ts: u64,
) -> Result<Identity, StoreError> {
    let folded = fold_email(email);
    claim_email(store, sub, &folded).await?;
    let mut identity = Identity::new(sub, display_name.map(|s| s.to_string()), created_ts);
    identity.email = Some(folded);
    let value = serde_json::to_value(&identity).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, IDENTITY_NS, IDENTITY_TABLE, sub, &value).await?;
    Ok(identity)
}

/// Read global identity `sub`, or `None` if it does not exist. Read-only (resolution path).
pub async fn identity_get(store: &Store, sub: &str) -> Result<Option<Identity>, StoreError> {
    let Some(value) = read(store, IDENTITY_NS, IDENTITY_TABLE, sub).await? else {
        return Ok(None);
    };
    let identity: Identity =
        serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(Some(identity))
}

/// Every global identity, sorted by `sub` for a stable listing (testing §3).
pub async fn identity_list(store: &Store) -> Result<Vec<Identity>, StoreError> {
    let rows = store_list(store, IDENTITY_NS, IDENTITY_TABLE, "kind", IDENTITY_KIND).await?;
    let mut identities: Vec<Identity> = rows
        .into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string())))
        .collect::<Result<_, _>>()?;
    identities.sort_by(|a, b| a.sub.cmp(&b.sub));
    Ok(identities)
}

/// The record stored at `identity_email:{folded_email}` — the reverse index owning `sub`. Just the
/// owner; the folded email is the record id, so it is never duplicated in the value.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EmailIndex {
    sub: String,
}

/// Set (or change) global identity `sub`'s email (email-login scope), keeping the `identity_email`
/// reverse index globally unique in a **race-safe** way. Steps:
///
/// 1. `create` the new index row `identity_email:{folded}` → `{sub}`. `create` errors with
///    [`StoreError::Conflict`] if the folded email already exists — so two identities racing for the
///    same email cannot both win (the check is the write, not a read-then-write). A `Conflict` where
///    the existing owner is ALREADY `sub` is idempotent success (re-setting the same email).
/// 2. On success, drop the identity's PREVIOUS email index row (if it had a different email), so an
///    email can be reassigned after a change without leaking the old claim.
/// 3. Upsert the `email` field on the identity record.
///
/// Returns `Conflict` iff the email is owned by a DIFFERENT identity. The identity must already exist.
pub async fn identity_set_email(
    store: &Store,
    sub: &str,
    email: &str,
) -> Result<Identity, StoreError> {
    let Some(mut identity) = identity_get(store, sub).await? else {
        return Err(StoreError::Decode(format!("no such identity: {sub}")));
    };
    let folded = fold_email(email);
    claim_email(store, sub, &folded).await?;
    // We now own `folded`. Release the identity's previous (different) email claim, if any.
    if let Some(prev) = identity.email.as_deref() {
        if prev != folded {
            let _ = lb_store::delete(store, IDENTITY_NS, IDENTITY_EMAIL_TABLE, prev).await;
        }
    }
    identity.email = Some(folded);
    let value = serde_json::to_value(&identity).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, IDENTITY_NS, IDENTITY_TABLE, sub, &value).await?;
    Ok(identity)
}

/// Race-safe claim of a folded email for `sub`: `create` the reverse-index row (Conflict-on-duplicate
/// is the uniqueness enforcement), treating a conflict WE already own as idempotent success. `Err`
/// (`Conflict`) iff a DIFFERENT identity owns the email.
async fn claim_email(store: &Store, sub: &str, folded: &str) -> Result<(), StoreError> {
    let index_value = serde_json::to_value(EmailIndex { sub: sub.to_string() })
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    match lb_store::create(store, IDENTITY_NS, IDENTITY_EMAIL_TABLE, folded, &index_value).await {
        Ok(()) => Ok(()),
        Err(StoreError::Conflict) => match email_owner(store, folded).await? {
            Some(owner) if owner == sub => Ok(()), // idempotent re-claim by the same identity
            _ => Err(StoreError::Conflict),
        },
        Err(e) => Err(e),
    }
}

/// Look up the `sub` that owns `email` (case-insensitive) via the reverse index — the login path's
/// email→principal resolution. `Ok(None)` if no identity claims it. Read-only.
pub async fn identity_by_email(store: &Store, email: &str) -> Result<Option<String>, StoreError> {
    email_owner(store, &fold_email(email)).await
}

/// Read the owner `sub` of an already-folded email from the reverse index.
async fn email_owner(store: &Store, folded: &str) -> Result<Option<String>, StoreError> {
    let Some(value) = read(store, IDENTITY_NS, IDENTITY_EMAIL_TABLE, folded).await? else {
        return Ok(None);
    };
    let index: EmailIndex =
        serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(Some(index.sub))
}
