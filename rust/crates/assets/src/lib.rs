//! Shared workspace assets — docs, skills, sharing relations, and extension install records,
//! all as **state** in the one datastore (README §6.1, §6.12, files + skills scopes).
//!
//! This crate is the *store side* of S4: the asset models and the raw `lb_store` verbs that
//! persist and read them, all workspace-namespaced (the hard wall, §7). It holds **no
//! authorization** — exactly like `lb_inbox`, these are the raw verbs the host's asset service
//! runs *after* `caps::check` and the membership/grant gate (capability-first, §3.5). The host
//! (`lb_host`) owns the three-gate chokepoint; this crate owns the shape and the persistence.
//!
//! Content lives **as a record value**, not a SurrealDB `DEFINE BUCKET` — buckets are not in
//! our embedded build (`kv-mem`; `DEFINE BUCKET` fails to parse — verified, files scope). The
//! verb shape is bucket-compatible (put/get/list opaque content by id) so the S7 swap to an
//! S3/GCS-backed bucket is config behind the same verb, not a re-cut. One datastore, no blob
//! service (§3.2).
//!
//! Verbs, one per file (FILE-LAYOUT §3):
//! - `doc` — [`Doc`] + [`put_doc`] / [`get_doc`] / [`list_docs`]
//! - `skill` — [`Skill`] + [`put_skill`] / [`get_skill`] / [`list_skills`]
//! - `relation` — the generic share/link/grant/member edge: [`relate`] / [`related`] /
//!   [`unrelate`] / [`list_related`]
//! - `install` — [`Install`] + [`record_install`] / [`read_install`]

mod doc;
mod install;
mod relation;
mod skill;

pub use doc::{get_doc, list_docs, put_doc, Doc, Visibility};
pub use install::{
    delete_install, list_installs, read_install, record_install, ExtUi, Install, Tier,
};
pub use relation::{list_related, list_skill_grants, relate, related, unrelate, Relation};
pub use skill::{get_skill, list_skills, put_skill, Skill};
