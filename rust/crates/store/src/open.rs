//! Open an embedded SurrealDB. Two engines are compiled into **every** node: `Mem` (in-memory,
//! for tests/dev — [`Store::memory`]) and `SurrealKv` (persistent on-disk — [`Store::open`]).
//! Which constructor a node calls is a **config** decision at boot (`LB_STORE_PATH`), never a
//! code branch on role (symmetric nodes, rule #1). The handle type is identical for both, so
//! every read/write/list/write_tx caller is unchanged above this seam.
//!
//! The persistent engine is **SurrealKV** (pinned by the S9 store spike: pure-Rust, no C++
//! toolchain, the "builds anywhere / on a Pi" posture; durability across restart and the
//! LOAD-BEARING feature set verified — see `docs/scope/store/persistent-backend-scope.md`).

use surrealdb::engine::local::{Db, Mem, SurrealKv};
use surrealdb::Surreal;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("store backend error: {0}")]
    Backend(String),
    #[error("value did not deserialize: {0}")]
    Decode(String),
}

impl From<surrealdb::Error> for StoreError {
    fn from(e: surrealdb::Error) -> Self {
        StoreError::Backend(e.to_string())
    }
}

/// A handle to the embedded datastore. Cloneable; cheap to pass around the host.
#[derive(Clone)]
pub struct Store {
    db: Surreal<Db>,
}

impl Store {
    /// Open an in-memory store (tests / dev). Each call is an isolated ephemeral instance — its
    /// data is gone when the handle drops. Use [`open`](Store::open) for a node that must survive
    /// a restart.
    pub async fn memory() -> Result<Self, StoreError> {
        let db = Surreal::new::<Mem>(()).await?;
        Ok(Self { db })
    }

    /// Open a **persistent** embedded store at `path` (a real node). Durable across restart:
    /// write, drop the handle, reopen at the same `path`, and the records are still there. This
    /// is the one thing `memory()` cannot do — the foundation of every must-deliver/ingest
    /// guarantee. The engine is SurrealKV; the namespace-per-workspace wall holds identically to
    /// the in-memory engine (all workspaces live in one on-disk store, scoped by `use_ns`).
    pub async fn open(path: &str) -> Result<Self, StoreError> {
        let db = Surreal::new::<SurrealKv>(path).await?;
        Ok(Self { db })
    }

    /// Bind the connection to a workspace's namespace (and a fixed database within it).
    /// Every read/write calls this first, so an operation can only ever touch its own
    /// workspace's namespace — the hard wall, structurally (README §7).
    pub(crate) async fn use_ws(&self, ws: &str) -> Result<&Surreal<Db>, StoreError> {
        self.db.use_ns(ws).use_db("main").await?;
        Ok(&self.db)
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
