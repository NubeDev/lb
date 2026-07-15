//! Open an embedded SurrealDB. Two engines are compiled into **every** node: `Mem` (in-memory,
//! for tests/dev — [`Store::memory`]) and `SurrealKv` (persistent on-disk — [`Store::open`]).
//! Which constructor a node calls is a **config** decision at boot (`LB_STORE_PATH`), never a
//! code branch on role (symmetric nodes, rule #1). The handle type is identical for both, so
//! every read/write/list/write_tx caller is unchanged above this seam.
//!
//! The persistent engine is **SurrealKV** (pinned by the S9 store spike: pure-Rust, no C++
//! toolchain, the "builds anywhere / on a Pi" posture; durability across restart and the
//! LOAD-BEARING feature set verified — see `docs/scope/store/persistent-backend-scope.md`).
//! Log compaction (boot-time and online) lives in `compact.rs`.

use std::ops::Deref;
use std::sync::Arc;

use surrealdb::engine::local::{Db, Mem, SurrealKv};
use surrealdb::Surreal;
use thiserror::Error;
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::compact::{compact_log, CompactionRecord};

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("store backend error: {0}")]
    Backend(String),
    #[error("value did not deserialize: {0}")]
    Decode(String),
    /// A first-write (`create`) hit an existing record at that id — the first write already bound
    /// (agent-run scope Part 2 first-settle). The caller treats this as "someone else decided
    /// first", not a backend failure.
    #[error("record already exists (first-write conflict)")]
    Conflict,
}

impl From<surrealdb::Error> for StoreError {
    fn from(e: surrealdb::Error) -> Self {
        StoreError::Backend(e.to_string())
    }
}

/// A handle to the embedded datastore. Cloneable; cheap to pass around the host.
///
/// **Session-namespace safety.** Every clone shares ONE embedded SurrealDB connection, and that
/// connection carries a single mutable session (its selected namespace + database). Selecting a
/// workspace's namespace (`use_ns(ws)`) is therefore a *global* mutation of that shared session,
/// not a per-operation scoping — and it is a distinct `await` from the query it is meant to guard.
/// On a multi-thread runtime (every node) two operations targeting different workspaces would
/// otherwise interleave (`use_ns(A)` … `use_ns(B)` … A's query runs against B's namespace),
/// breaking the workspace wall non-deterministically (it surfaced as the flaky login "not a member
/// of any workspace" — a bootstrap membership written into one namespace, read back from another;
/// see debugging/store/concurrent-use-ns-namespace-race.md). The [`session`](Store::session) mutex
/// makes the `use_ns` + query pair a critical section: only one namespace-scoped operation touches
/// the shared session at a time, so a query always runs against the namespace *it* selected.
///
/// **The mutex CARRIES the handle** (online-compaction scope): the `Surreal<Db>` lives *inside*
/// the session mutex, so the same critical section that guards `use_ns`+query also guards the
/// handle swap the online compaction pass performs (drop → compact on disk → reopen → swap back).
/// Holding the lock means holding the one true handle; there is no window where a query can run
/// against a half-open engine.
#[derive(Clone)]
pub struct Store {
    /// The ONE shared connection, behind the ONE session lock. Held (via [`WsGuard`]) from
    /// `use_ws` until the caller's query completes. See the type-level note above.
    session: Arc<Mutex<Surreal<Db>>>,
    /// The on-disk directory for a persistent store; `None` for `memory()` (which cannot
    /// compact — there is no log). Used by `compact`/`status`, never by the data verbs.
    path: Option<Arc<str>>,
    /// Outcome of the most recent compaction pass (boot or online), served by `status`.
    /// In-memory only — a restart re-seeds it from the boot pass.
    last_compaction: Arc<std::sync::Mutex<Option<CompactionRecord>>>,
}

/// The namespace-scoped session, held for the duration of one store operation. Owning this guard
/// means the shared connection's session is bound to *this* operation's workspace and cannot be
/// re-pointed by a concurrent task until the guard drops — and (since the mutex carries the
/// handle) that the engine cannot be swapped out from under the query either. Deref to
/// `&Surreal<Db>` so callers run their query exactly as before.
pub(crate) struct WsGuard {
    guard: OwnedMutexGuard<Surreal<Db>>,
}

impl Deref for WsGuard {
    type Target = Surreal<Db>;
    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl Store {
    /// Open an in-memory store (tests / dev). Each call is an isolated ephemeral instance — its
    /// data is gone when the handle drops. Use [`open`](Store::open) for a node that must survive
    /// a restart.
    pub async fn memory() -> Result<Self, StoreError> {
        let db = Surreal::new::<Mem>(()).await?;
        Ok(Self {
            session: Arc::new(Mutex::new(db)),
            path: None,
            last_compaction: Arc::new(std::sync::Mutex::new(None)),
        })
    }

    /// Open a **persistent** embedded store at `path` (a real node). Durable across restart:
    /// write, drop the handle, reopen at the same `path`, and the records are still there. This
    /// is the one thing `memory()` cannot do — the foundation of every must-deliver/ingest
    /// guarantee. The engine is SurrealKV; the namespace-per-workspace wall holds identically to
    /// the in-memory engine (all workspaces live in one on-disk store, scoped by `use_ns`).
    ///
    /// The commit log is compacted first (see [`compact_log`]) — SurrealKV is append-only and
    /// replays every byte of the log at open, so a long-running node otherwise pays its whole
    /// write history on every boot (measured: a 1.5 GB log ≈ 13 s to open, live set ~2% of it).
    pub async fn open(path: &str) -> Result<Self, StoreError> {
        let owned = path.to_string();
        // `compact()` is synchronous file I/O over the whole log — keep it off the async
        // workers. Best-effort by design: a failed compaction only means a slower boot.
        let boot_pass = tokio::task::spawn_blocking(move || compact_log(&owned))
            .await
            .ok();
        let db = Surreal::new::<SurrealKv>(path).await?;
        Ok(Self {
            session: Arc::new(Mutex::new(db)),
            path: Some(Arc::from(path)),
            last_compaction: Arc::new(std::sync::Mutex::new(boot_pass)),
        })
    }

    /// Bind the shared connection to a workspace's namespace (and a fixed database within it) and
    /// return a guard that holds the session lock for the duration of the caller's query. Every
    /// read/write calls this first, so an operation can only ever touch its own workspace's
    /// namespace — the hard wall, structurally (README §7) — and, because the guard holds the lock
    /// across the query, a concurrent operation for another workspace cannot re-point the shared
    /// session mid-query (see the [`Store`] type note).
    pub(crate) async fn use_ws(&self, ws: &str) -> Result<WsGuard, StoreError> {
        // Acquire the session lock BEFORE selecting the namespace, and hold it (via the returned
        // guard) across the caller's query. The guard owns the mutex that carries the handle, so
        // the borrow-vs-return dance the old plain-field layout needed is gone.
        let guard = Arc::clone(&self.session).lock_owned().await;
        guard.use_ns(ws).use_db("main").await?;
        Ok(WsGuard { guard })
    }

    /// The session mutex + handle cell, for the online compaction pass only (`compact.rs`).
    pub(crate) fn session_cell(&self) -> Arc<Mutex<Surreal<Db>>> {
        Arc::clone(&self.session)
    }

    /// The on-disk directory (`None` for a memory store).
    pub(crate) fn dir(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// The last-compaction slot (`compact.rs` writes it; `status.rs` reads it).
    pub(crate) fn last_compaction_slot(&self) -> &std::sync::Mutex<Option<CompactionRecord>> {
        &self.last_compaction
    }

    /// Run a raw SurrealQL statement, returning the response for the caller to extract. The
    /// **escape hatch** for the day-one capability spike and for callers (ingest, tags) that need
    /// `RELATE`/`DEFINE`/composite-ID statements the generic key-value verbs do not express. The
    /// namespace is selected from `ws` first — the same hard wall as every other verb. This is a
    /// raw verb run *after* `caps::check`; it is not an authorization point.
    pub async fn query_ws(
        &self,
        ws: &str,
        sql: &str,
        bindings: Vec<(String, serde_json::Value)>,
    ) -> Result<surrealdb::Response, StoreError> {
        let db = self.use_ws(ws).await?;
        let mut q = db.query(sql);
        for (k, v) in bindings {
            q = q.bind((k, v));
        }
        Ok(q.await?.check()?)
    }
}
