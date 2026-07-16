//! The extension install record — the persisted `requested ∩ approved` grant set for an
//! extension in a workspace (README §6.4, extensions scope). S1 passed `admin_approved` in by
//! the caller on every `load_extension`; S4 persists the approved set so it survives a restart
//! and is the durable source of truth for what an extension is allowed (the extensions-scope
//! open question: "where the admin-approval set is stored").
//!
//! It is a workspace asset like any other: namespace-scoped (README §7), addressed by the
//! extension id, raw verbs the host loader reads. One verb per file (FILE-LAYOUT §3).

mod delete;
mod list;
mod model;
mod read;
mod record;

pub use delete::delete_install;
pub use list::list_installs;
pub use model::{ExtUi, ExtUiOption, Install, Tier};
pub use read::read_install;
pub use record::record_install;

/// The store table install records live in, within a workspace namespace.
pub(crate) const TABLE: &str = "install";
