//! The **session store** (browser-session scope): `sid → {token, principal, ws, expires_at}`, held in
//! the node's own SurrealDB so sessions survive a restart. The dev plugins this replaces keep a
//! process-local `Map` — correct for a dev seam, wrong for a product: every deploy would log everyone
//! out (scope → Goals, "sessions survive a restart").
//!
//! **Namespace.** Session rows are looked up by `sid` alone — the cookie is all the browser sends, and
//! the workspace is what we are trying to *learn*. So they cannot live in a workspace-scoped table.
//! They go in the reserved system namespace `_lb_browser_session`, the same convention `lb_authz`'s
//! identity directory uses for genuinely global records (`IDENTITY_NS = "_lb_identity"`, and
//! `_lb_workspaces` / `_lb_workflow_directory` before it): a leading `_lb_` marks it system-internal,
//! and an operator must never name a real workspace this.
//!
//! **This does not weaken the workspace wall.** The row is a *lookup*, not an authority: it stores the
//! token the caller already earned, and every `/api/*` request re-presents that token to the same
//! guarded route a CLI would hit. The `ws` field is a fact ABOUT the session, never a grant — the wall
//! is enforced downstream by the token's own `ws` claim, exactly as for a bearer caller.
//!
//! **The token is data at rest here.** That is a deliberate, scoped trade (it already is in the dev
//! map): the store is the more defensible home than the browser, which is the whole point — the JWT is
//! ~4–9KB of full cap set and must never reach JS.

use lb_store::{delete, read, write, Store, StoreError};
use serde::{Deserialize, Serialize};

/// The reserved system namespace browser sessions live in. Leading `_lb_` marks it system-internal
/// (the `IDENTITY_NS` convention); operators must not name a real workspace this.
pub const SESSION_NS: &str = "_lb_browser_session";

/// The table within that namespace.
pub const SESSION_TABLE: &str = "session";

/// A stored browser session. The `token` is the bearer the sid stands in for; everything else is the
/// public fact set the shell is allowed to see.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRow {
    /// The gateway JWT this sid resolves to. **Never** serialized to an `/api/*` response.
    pub token: String,
    /// The canonical `user:<handle>` principal the session authenticated as.
    pub principal: String,
    /// The workspace the token is minted into (a fact, not an authority — see the module note).
    pub ws: String,
    /// Absolute expiry, seconds, on the gateway clock. Enforced on every read.
    pub expires_at: u64,
}

/// Persist a session under `sid`.
pub async fn put(store: &Store, sid: &str, row: &SessionRow) -> Result<(), StoreError> {
    let value = serde_json::to_value(row).expect("SessionRow serializes");
    write(store, SESSION_NS, SESSION_TABLE, sid, &value).await
}

/// Resolve `sid` → its session, **enforcing the TTL**: an expired row reads as `None` and is deleted
/// on the way out, so a stale cookie is indistinguishable from an unknown one (no oracle) and the row
/// does not linger. `now` is the gateway clock.
pub async fn get(store: &Store, sid: &str, now: u64) -> Result<Option<SessionRow>, StoreError> {
    let Some(value) = read(store, SESSION_NS, SESSION_TABLE, sid).await? else {
        return Ok(None);
    };
    // A row that will not deserialize is a corrupt/foreign record, not a session — treat it as absent
    // rather than 500 (scope: "never a 500, never an anonymous pass-through").
    let Ok(row) = serde_json::from_value::<SessionRow>(value) else {
        return Ok(None);
    };
    if row.expires_at <= now {
        // Best-effort GC of the expired row; the answer is `None` either way.
        let _ = delete(store, SESSION_NS, SESSION_TABLE, sid).await;
        return Ok(None);
    }
    Ok(Some(row))
}

/// Drop a session (logout, or the old sid after a rotation). Idempotent.
pub async fn remove(store: &Store, sid: &str) -> Result<(), StoreError> {
    delete(store, SESSION_NS, SESSION_TABLE, sid).await
}
