//! The document asset — a workspace-scoped doc with content + metadata, persisted as state
//! (README §6.12, files scope). One verb per file (FILE-LAYOUT §3): the [`Doc`] model, then
//! [`put_doc`] (write), [`get_doc`] (point read), [`list_docs`] (the owner's docs).
//!
//! No authorization here — these are raw store verbs. The host's asset service runs the
//! capability check (`store:doc/*:…`) and the membership gate (owner / shared-team /
//! linked-channel) before calling them.

mod delete;
mod get;
mod list;
mod model;
mod put;

pub use delete::delete_doc;
pub(crate) use delete::TOMBSTONE;
pub use get::get_doc;
pub use list::list_docs;
pub use model::{ContentType, Doc, Visibility};
pub use put::put_doc;

/// The store table all doc assets live in, within a workspace namespace.
pub(crate) const TABLE: &str = "doc";
