//! Open an embedded SurrealDB. S1 uses the in-memory engine (`mem://`); the same handle type
//! backs a file/rocksdb engine later by config (symmetric nodes — the engine is config, not
//! code). One shared instance with namespace-per-workspace isolation (core scope open Q).

use surrealdb::engine::local::{Db, Mem};
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
    /// Open an in-memory store (S1 / tests). Each call is an isolated ephemeral instance.
    pub async fn memory() -> Result<Self, StoreError> {
        let db = Surreal::new::<Mem>(()).await?;
        Ok(Self { db })
    }

    /// Bind the connection to a workspace's namespace (and a fixed database within it).
    /// Every read/write calls this first, so an operation can only ever touch its own
    /// workspace's namespace — the hard wall, structurally (README §7).
    pub(crate) async fn use_ws(&self, ws: &str) -> Result<&Surreal<Db>, StoreError> {
        self.db.use_ns(ws).use_db("main").await?;
        Ok(&self.db)
    }
}
