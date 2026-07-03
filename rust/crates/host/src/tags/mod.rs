//! The tags service — the host's capability chokepoint for the typed annotation + relationship graph
//! (README §6.11, tags scope). Wraps the raw `lb_tags` graph verbs with the gate (capability-first
//! §3.5, isolation-first §3.6) and the required per-workspace tag-node cap.
//!
//! Verbs (one per concern, FILE-LAYOUT): `tags.add` / `tags.remove` / `tags.of` / `tags.find` —
//! and nothing else (event registration is host-internal; no caller-facing verb). The MCP bridge
//! ([`call_tags_tool`]) exposes them under the one MCP contract.

mod authorize;
mod error;
mod tool;
mod verbs;

pub use authorize::authorize_tags;
pub use error::TagsError;
pub use tool::call_tags_tool;
pub use verbs::{tags_add, tags_facet_values, tags_find, tags_of, tags_remove};

// Re-export the graph value types so host callers / tests use one set.
pub use lb_tags::{Applied, Facet, Provenance, Source, Tag};
