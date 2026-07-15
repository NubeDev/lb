//! Open an embedded SurrealDB. Two engines are compiled into **every** node: `Mem` (in-memory,
//! for tests/dev — [`Store::memory`]) and `SurrealKv` (persistent on-disk — [`Store::open`]).
//! Which constructor a node calls is a **config** decision at boot (`LB_STORE_PATH`), never a
//! code branch on role (symmetric nodes, rule #1). The handle type is identical for both, so
//! every read/write/list/write_tx caller is unchanged above this seam.
//!
//! The persistent engine is **SurrealKV** (pinned by the S9 store spike: pure-Rust, no C++
//! toolchain, the "builds anywhere / on a Pi" posture; durability across restart and the
//! LOAD-BEARING feature set verified — see `docs/scope/store/persistent-backend-scope.md`).

use std::ops::Deref;
use std::sync::Arc;

use surrealdb::engine::local::{Db, Mem, SurrealKv};
use surrealdb::Surreal;
use thiserror::Error;
use tokio::sync::{Mutex, OwnedMutexGuard};

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
#[derive(Clone)]
pub struct Store {
    db: Surreal<Db>,
    /// Serializes the "select namespace, then run the statement" window across all clones (they
    /// share one connection + one session). Held from `use_ws` until the returned guard drops —
    /// i.e. across the caller's query. See the type-level note above.
    session: Arc<Mutex<()>>,
}

/// The namespace-scoped session, held for the duration of one store operation. Owning this guard
/// means the shared connection's session is bound to *this* operation's workspace and cannot be
/// re-pointed by a concurrent task until the guard drops. Deref to `&Surreal<Db>` so callers run
/// their query exactly as before — the only change is that the query now runs *while holding* the
/// lock, closing the `use_ns`↔query race.
pub(crate) struct WsGuard<'a> {
    db: &'a Surreal<Db>,
    // The lifetime of the critical section. Dropped after the caller's query completes; `_guard`
    // is never read, only held.
    _guard: OwnedMutexGuard<()>,
}

impl Deref for WsGuard<'_> {
    type Target = Surreal<Db>;
    fn deref(&self) -> &Self::Target {
        self.db
    }
}

impl Store {
    /// Open an in-memory store (tests / dev). Each call is an isolated ephemeral instance — its
    /// data is gone when the handle drops. Use [`open`](Store::open) for a node that must survive
    /// a restart.
    pub async fn memory() -> Result<Self, StoreError> {
        let db = Surreal::new::<Mem>(()).await?;
        Ok(Self {
            db,
            session: Arc::new(Mutex::new(())),
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
        let _ = tokio::task::spawn_blocking(move || compact_log(&owned)).await;
        let db = Surreal::new::<SurrealKv>(path).await?;
        Ok(Self {
            db,
            session: Arc::new(Mutex::new(())),
        })
    }

    /// Bind the shared connection to a workspace's namespace (and a fixed database within it) and
    /// return a guard that holds the session lock for the duration of the caller's query. Every
    /// read/write calls this first, so an operation can only ever touch its own workspace's
    /// namespace — the hard wall, structurally (README §7) — and, because the guard holds the lock
    /// across the query, a concurrent operation for another workspace cannot re-point the shared
    /// session mid-query (see the [`Store`] type note).
    pub(crate) async fn use_ws(&self, ws: &str) -> Result<WsGuard<'_>, StoreError> {
        // Acquire the session lock BEFORE selecting the namespace, and hold it (via the returned
        // guard) across the caller's query. `OwnedMutexGuard` would need `'static`; we borrow the
        // `Arc` and lock it, then bind that guard's lifetime to `&self` through the plain guard —
        // but a plain `MutexGuard<'_>` borrows `self.session`, which conflicts with returning
        // `&self.db`. Cloning the `Arc` and taking an owned guard sidesteps the borrow entirely.
        let guard = Arc::clone(&self.session).lock_owned().await;
        self.db.use_ns(ws).use_db("main").await?;
        Ok(WsGuard {
            db: &self.db,
            _guard: guard,
        })
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

/// Compact the SurrealKV commit log at `path` before SurrealDB opens it.
///
/// The engine is append-only: every write (including each superseded version and every
/// tombstone) stays in the log forever, and open replays ALL of it to rebuild the in-memory
/// index — so boot time grows with write history, not with live data. SurrealKV ships
/// `Store::compact()` (rewrites only the latest live versions, drops tombstoned/superseded
/// records), but surrealdb 2.x exposes no path to it, so it is invoked here directly, on the
/// same engine version cargo resolves for surrealdb (one `surrealkv` copy in the lock).
///
/// Options mirror surrealdb's own wrapper (`surrealdb-core/src/kvs/surrealkv/mod.rs` — the
/// unversioned `surrealkv://` scheme lb uses): versions off, disk persistence on, 512 MiB
/// segments, 64-byte value threshold. SurrealKV persists options in its manifest and merges
/// on load, so the cache-size knob (runtime-only) is left at its default.
///
/// Best-effort by contract: any failure leaves the log exactly as it was and only costs a
/// slower boot, so errors are reported to stderr and swallowed. A fresh path (no store yet)
/// is skipped outright.
fn compact_log(path: &str) {
    let dir = std::path::Path::new(path);
    if !dir.exists() {
        return;
    }
    let mut opts = surrealkv::Options::new();
    opts.dir = dir.to_path_buf();
    opts.disk_persistence = true;
    opts.enable_versions = false;
    opts.max_segment_size = 1 << 29;
    opts.max_value_threshold = 64;
    let store = match surrealkv::Store::new(opts) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("store: open-for-compaction failed ({e}) — booting on the uncompacted log");
            return;
        }
    };
    if let Err(e) = store.compact() {
        eprintln!("store: log compaction failed ({e}) — booting on the uncompacted log");
    }
    if let Err(e) = store.close() {
        eprintln!("store: closing the compaction handle failed: {e}");
    }
}
