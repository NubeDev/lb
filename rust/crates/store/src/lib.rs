//! The datastore — embedded SurrealDB, the one source of truth on every node (README §6.1).
//!
//! Tenancy mapping (§7): **workspace = SurrealDB namespace**. A [`Store`] handle is opened
//! once; each operation is scoped to a workspace, which selects the namespace before the
//! query runs. That makes workspace isolation *structural* at the store layer — a query for
//! workspace A physically cannot read namespace B's records.
//!
//! State only (§3.3): the store holds state; motion is the bus's job. No pub/sub here.

mod boot_guard;
mod capped;
mod compact;
mod compaction_record;
mod conflict;
mod create;
mod delete;
mod engine_open;
mod graph;
mod increment;
mod last_pass;
mod list;
mod meminfo;
mod null_option;
mod open;
mod read;
mod read_versioned;
mod reader;
mod record;
pub mod reserved;
mod scan;
mod scan_all;
mod scoped_response;
pub mod secret_tables;
mod snapshot_guard;
mod status;
mod tables;
mod taint;
mod write;
mod write_batch;
mod write_journaled;
mod write_locked;
mod write_tx;

pub use boot_guard::{is_productive, PRODUCTIVE_RECLAIM_RATIO};
pub use capped::{capped_insert, new_ulid, ulid_timestamp_ms};
pub use null_option::null_as_none;
/// Re-exported so crates that deserialise query rows do NOT need a direct `surrealdb`
/// dependency — the wrapper boundary stays at `store` + `host` (it was 2 crates before
/// SurrealDB 3 and must stay 2 after). Downstream crates derive with
/// `#[derive(SurrealValue)] #[surreal(crate = "lb_store::surreal_types")]`.
pub use surrealdb::types as surreal_types;

/// Implement `SurrealValue` for a type by **delegating to its existing serde impls**.
///
/// SurrealDB 3 deserialises through `SurrealValue`, whose derive supports `rename`, `rename_all`,
/// `skip` and `wrap` — but **not `default`, `deserialize_with` or `skip_serializing_if`**. Any type
/// that relies on those would silently change meaning under a plain `#[derive(SurrealValue)]`.
///
/// The sharpest example is `lb_ingest::Policy::max_samples`, which carries
/// `#[serde(default, deserialize_with = "none_as_default")]` because a row written before that field
/// existed reads back as a present-but-null `NONE` — which `default` never sees. `run_gc` opens with
/// `list_policies`, so ONE such row aborted a whole workspace's retention pass. A derive that
/// dropped that guard would reintroduce the bug, and it would compile cleanly.
///
/// This macro routes the type through `SerdeWrapper`, so `Serialize`/`Deserialize` — and every
/// attribute on them — remain the single source of truth.
///
/// # When to reach for this instead of the derive
///
/// **If a row was WRITTEN as JSON, read it back with serde.** Almost every lb write binds
/// `serde_json::Value` params, so the bytes on disc are serde's shape, and the derive's own shape is
/// a different one. Three distinct ways that bit us during the SurrealDB 3 migration, each a silent
/// compile-clean failure that only appeared when a real row was read:
///
///   1. **Dropped attributes.** `lb_store::Record::rev` carries `#[serde(default = "default_rev")]`;
///      the derive ignores serde attributes, so every pre-`rev` row failed with "Expected number,
///      got none" instead of reading as 1.
///   2. **A different enum wire form.** `lb_tags::Source` is stored as a bare lowercase string
///      (`add.rs` binds `source.as_str()`). The derive encodes enums its own way and rejected every
///      stored row: "Failed to decode Source, no variants matched".
///   3. **NULL is not NONE.** `lb_tags::Applied::expires` is `Option<u64>` written as JSON, so an
///      absent value lands as SQL NULL. The derive's `Option` accepts NONE and refused NULL:
///      "Expected number, got null". serde reads null as `None`.
///
/// The derive is fine for a type that never round-trips through JSON — a projection row built and
/// consumed inside one query (`CountRow { c: u64 }`, `IdRow { id: String }`). Anything with
/// `Option`, a serde attribute, or an enum should come through here.
#[macro_export]
macro_rules! surreal_value_via_serde {
    ($($t:ty),* $(,)?) => {$(
        impl $crate::surreal_types::SurrealValue for $t {
            fn kind_of() -> $crate::surreal_types::Kind {
                <$crate::surreal_types::SerdeWrapper<$t> as $crate::surreal_types::SurrealValue>::kind_of()
            }
            fn is_value(v: &$crate::surreal_types::Value) -> bool {
                <$crate::surreal_types::SerdeWrapper<$t> as $crate::surreal_types::SurrealValue>::is_value(v)
            }
            fn into_value(self) -> $crate::surreal_types::Value {
                $crate::surreal_types::SurrealValue::into_value(
                    $crate::surreal_types::SerdeWrapper(self))
            }
            fn from_value(v: $crate::surreal_types::Value)
                -> ::std::result::Result<Self, $crate::surreal_types::Error> {
                <$crate::surreal_types::SerdeWrapper<$t> as $crate::surreal_types::SurrealValue>
                    ::from_value(v).map(|w| w.0)
            }
        }
    )*};
}
pub use compact::compact;
pub use compaction_record::{CompactionPhases, CompactionRecord};
pub use create::create;
pub use delete::delete;
pub use graph::{graph, Edge as GraphEdge, Graph, Node as GraphNode, MAX_FANOUT, MAX_SEED};
pub use increment::increment;
pub use last_pass::last_persisted_compaction;
pub use list::list;
pub use meminfo::available_ram_bytes;
pub use open::{OpenOptions, Store, StoreError};
pub use read::read;
pub use read_versioned::read_versioned;
pub use record::{Versioned, FIRST_REV};
pub use reserved::{is_reserved, RESERVED_TABLES};
pub use scan::{scan, Page, Row, MAX_SCAN_LIMIT};
pub use scan_all::scan_all;
pub use secret_tables::{is_secret_table, secret_table_of, SECRET_TABLES};
pub use snapshot_guard::{snapshot_safety, SnapshotRefusal};
pub use status::{status, StoreStatus};
pub use surrealdb::types::SurrealValue;
pub use tables::{tables, TableCount};
pub use taint::{
    mark_outbox_reached, mark_store_written, outbox_was_reached, store_was_written, taint_scope,
    TaintVerdict,
};
pub use write::write;
pub use write_batch::{write_batch, DeleteBatch, UpsertBatch, MAX_BATCH};
pub use write_journaled::{write_journaled, JournalWrite};
pub use write_locked::write_locked;
pub use write_tx::{write_tx, Upsert};
