//! `lb-tags` — a typed annotation + relationship graph, SurrealDB-native (README §6.11, tags scope).
//!
//! A tag is **not a string** — it is a shared, typed **node** (`tag:[key,value]`, composite id), and
//! *applying* a tag is a **provenance-carrying edge** (`RELATE entity -> tagged -> tag`). One
//! subsystem serves four jobs: labeling, provenance, lineage, classification — the connective tissue
//! that makes heterogeneous data (ingest series, inbox items, files, jobs) navigable by meaning and
//! relationship instead of by schema.
//!
//! **CORE (ships unconditionally):** the graph model + composite-ID exact lookup + key-only +
//! faceted traversal + `add`/`remove`/`of`/`find`, plus the required per-workspace tag-node cap.
//! **SPIKE-GATED ADD-ONS (all available per the store spike matrix):** value full-text (`SEARCH`),
//! vector (`HNSW`), and materialized per-dimension count views (`tag_counts`).
//!
//! Edge identity is `(entity, tag, source)` — same-source re-tag upserts; different sources coexist.
//! `DEFINE EVENT` is host-internal only (never a caller verb). Authorization is NOT here — raw verbs
//! run after `caps::check` (the host tags service is the chokepoint, capability-first §3.5).

mod add;
mod cap;
mod counts;
mod edge;
mod entity;
mod find;
mod of;
mod remove;
mod search;
mod tag;
mod values;
mod vector;

pub use add::{add, AddError};
pub use cap::{check_cap, CapExceeded, DEFAULT_TAG_NODE_CAP};
pub use counts::{count_by_key, define_counts_view, KeyCount};
pub use edge::{Provenance, Source, TAGGED_TABLE};
pub use entity::entity_parts;
pub use find::{find, Facet};
pub use of::{of, Applied};
pub use remove::remove;
pub use search::{define_text_index, find_text};
pub use tag::{Tag, TAG_TABLE};
pub use values::facet_values;
pub use vector::{define_vector_index, find_similar, put_vector, DimMismatch, VECTOR_TABLE};
