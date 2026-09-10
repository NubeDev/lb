//! The **read-only handle** behind `store.query` — one per workspace, enforced by the engine.
//!
//! # Why this exists
//!
//! `store.query` runs SurrealQL the caller wrote. Under SurrealDB 2 the safety came from a
//! host-side gate: parse the SQL with SurrealDB's own parser and allowlist a single
//! `SELECT`/`INFO`/`SHOW`. SurrealDB 3 sealed the predicate that gate depended on
//! (`TopLevelExpr::read_only` and friends are `pub(crate)`), so there is no longer a supported way
//! to ask "does this statement write?" before running it.
//!
//! This replaces the question with a stronger one: **do not ask, make it impossible.** The query
//! runs in a session the engine itself will not let write. The guarantee stops depending on our
//! reading of the caller's SQL, which could always drift from the grammar that actually executes.
//!
//! # Why DATABASE level, not ROOT
//!
//! `Role::Viewer` at ROOT level blocks writes but reads **every namespace** — measured, see
//! `tests/viewer_namespace_probe.rs`. That would have dissolved the workspace wall the moment the
//! parse gate stopped rejecting a smuggled `USE NS other-ws;`, because the wall is a prepended
//! `USE` and nothing stopped the caller writing their own.
//!
//! Signing in at DATABASE level for `<ws>/main` closes it structurally: `ctx::check_perms`
//! (surrealdb-core `src/ctx/context.rs:612`) permits the bypass only when the actor's own level
//! covers the namespace being touched, so a ws-A reader that reaches for ws-B falls through to
//! table permissions and comes back empty. The wall is then enforced twice — by the prepended `USE`
//! and by the engine — and neither depends on inspecting the caller's text.
//!
//! # What a refused write looks like
//!
//! `Ok([])` — an empty result, **not** an error (measured, `tests/viewer_session_probe.rs`). The
//! data is unchanged, which is the property that matters. `store.query` adds a separate advisory
//! message so a caller who sends a write is told, rather than left reading zero rows; that message
//! is ergonomics, and this file is the security.
//!
//! # Cost
//!
//! `signin` verifies a password hash and is deliberately slow, so a handle is built once per
//! workspace and cached. Handles stay valid for the life of the `Store`: compaction no longer swaps
//! the engine (`compact.rs` is a no-op under the LSM engine), so there is nothing to invalidate.

use std::collections::HashMap;

use surrealdb::engine::local::Db;
use surrealdb::opt::auth::Database;
use surrealdb::Surreal;
use tokio::sync::RwLock;

use crate::open::StoreError;

/// The one username, defined per workspace database. It is not a secret: the password is.
const READER_USER: &str = "lb_query_reader";

/// Per-workspace read-only handles, built on first use.
#[derive(Debug, Default)]
pub(crate) struct Readers {
    by_ws: RwLock<HashMap<String, Surreal<Db>>>,
}

impl Readers {
    /// The read-only handle for `ws`, building and caching it on first use.
    ///
    /// Two callers racing on the same new workspace both take the write lock; the second finds the
    /// first's handle and returns it, so `signin` is paid once. Holding the write lock across the
    /// build serializes only the first use of each workspace, never a query.
    pub(crate) async fn get_or_build(
        &self,
        root: &Surreal<Db>,
        ws: &str,
        secret: &str,
    ) -> Result<Surreal<Db>, StoreError> {
        // Each caller gets a CLONE, which is a new session seeded from the cached one: the auth
        // carries over (so the expensive `signin` is paid once per workspace) but the session state
        // does not persist back.
        //
        // That is not caution, it is required. Session state is per-session and a caller's SQL can
        // set it — `LET $x = …` binds a parameter for the rest of that session. Sharing one handle
        // would let one caller's `LET` be visible to the next caller's query on the same workspace:
        // a cross-caller leak through a channel neither of them can see. A clone is two channel
        // messages and is released on drop (`impl Drop for Surreal` sends `SessionId::Drop`).
        if let Some(h) = self.by_ws.read().await.get(ws) {
            return Ok(h.clone());
        }
        let mut map = self.by_ws.write().await;
        if let Some(h) = map.get(ws) {
            return Ok(h.clone());
        }
        let handle = build(root, ws, secret).await?;
        map.insert(ws.to_string(), handle.clone());
        Ok(handle)
    }
}

/// Define the workspace's viewer and sign a fresh session in as it.
///
/// `OVERWRITE` rather than `IF NOT EXISTS`: the password is regenerated every boot and never
/// persisted anywhere we could read back, so a definition surviving on disc from a previous boot
/// carries a hash this process cannot produce. `IF NOT EXISTS` would keep that stale hash and every
/// signin would fail — a store that had been opened once would permanently refuse `store.query`.
async fn build(root: &Surreal<Db>, ws: &str, secret: &str) -> Result<Surreal<Db>, StoreError> {
    // `ws` reaches here already validated by `scope_sql`'s charset check, and is backtick-quoted for
    // the same reason: a legal slug like `ws-a` is not a bare SurrealQL identifier.
    //
    // The password is INTERPOLATED, not bound: SurrealQL's `DEFINE USER ... PASSWORD` takes a
    // string literal and rejects a `$param` at parse time. Interpolating a secret into SQL is only
    // safe if the secret provably cannot close the quote, so that is checked here rather than
    // assumed — `new_secret` returns Crockford base32, but this function must hold for any caller.
    if !secret.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(StoreError::Backend(
            "internal: the store reader secret must be alphanumeric".into(),
        ));
    }
    let define = format!(
        "USE NS `{ws}` DB main;\n\
         DEFINE USER OVERWRITE {READER_USER} ON DATABASE PASSWORD '{secret}' ROLES VIEWER;"
    );
    root.query(define)
        .await
        .map_err(|e| StoreError::Backend(e.to_string()))?
        .check()
        // The statement text carries the password, so a backend error is reported WITHOUT it.
        .map_err(|_| {
            StoreError::Backend(format!(
                "could not define the read-only user for workspace {ws:?}"
            ))
        })?;

    let session = root.clone();
    session
        .signin(Database {
            namespace: ws.to_string(),
            database: "main".to_string(),
            username: READER_USER.to_string(),
            password: secret.to_string(),
        })
        .await
        // The password must never reach a log or an error string, so this reports the failure
        // without the credential that caused it.
        .map_err(|e| {
            StoreError::Backend(format!(
                "could not open a read-only session for workspace {ws:?}: {e}"
            ))
        })?;
    Ok(session)
}

/// A per-boot secret for the reader users. In memory only: never logged, never written to the
/// store in clear, never derived from anything an operator sets.
///
/// Two ULIDs give 160 bits, of which 160 - 2*48 = 64 are the timestamp; the remaining 160 bits
/// include 2 * 80 bits of randomness. That guards a credential which never leaves this process and
/// which, if it did leak, grants read-only access to one workspace of an embedded database with no
/// listener — so the bound is comfortable and the dependency is one the crate already has.
pub(crate) fn new_secret() -> String {
    format!("{}{}", ulid::Ulid::new(), ulid::Ulid::new())
}
