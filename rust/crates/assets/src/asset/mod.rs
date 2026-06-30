//! The binary-asset store side — a workspace-scoped image / attachment, persisted as state
//! (document-store scope move 2: "land the file store for binaries"). Same shape discipline
//! as `doc`: raw `lb_store` verbs, no authorization here. One verb per file (FILE-LAYOUT §3).
//!
//! Bytes live **as a record value**, not a SurrealDB `DEFINE BUCKET` (kv-mem could not, and
//! SurrealKV's bucket support is the scope's open question — either way the verb takes/returns
//! **opaque bytes**, so the physical backing is config behind the same verb, never a leaked
//! SurrealDB-specific type). v1 bounds the size inline (document-store scope risk: "state the
//! bound explicitly, reject over-bound puts, never truncate silently"); the bound lives in the
//! host verb (`put_asset`), not here, so this crate stays pure shape + persistence.

mod delete;
mod get;
mod list;
mod model;
mod put;

pub use delete::delete_asset;
pub use get::get_asset;
pub use list::list_assets;
pub use model::Asset;
pub use put::put_asset;

/// The store table all binary-asset records live in, within a workspace namespace.
pub(crate) const TABLE: &str = "asset";
