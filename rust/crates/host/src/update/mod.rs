//! `update.*` — the mediated surface for a node that can replace itself (node-update scope §Seam 1).
//!
//! An operator sees "you are on 0.1.0, 0.1.2 is available" and applies it from the app the node
//! itself serves, behind the same caps wall and the same MCP contract as every other verb.
//!
//! **lb performs no update and stores no artifact.** The mechanism is the embedder's, injected as
//! `BootConfig.update: Option<UpdateConfig>` carrying an `Arc<dyn UpdateProvider>` — no core crate
//! names a supervisor, a package format, or a product (rule 10). No provider ⇒ `update.status`
//! answers `{"supported": false}` and every other verb is a clean `Unsupported`.

mod apply;
pub mod audit;
mod context;
mod credential;
mod enrol;
mod error;
mod installed;
mod model;
mod provider;
mod read;
mod tool;

pub use credential::{fingerprint, HOST_SUBJECT};
pub use error::{UpdateError, UNSUPPORTED_PREFIX};
pub use installed::InstalledUpdate;
pub use model::{
    Accepted, AvailableVersion, CredentialSource, CredentialStatus, UpdateEvent, UpdateOutcome,
    UpdateStatus,
};
pub use provider::{UpdateConfig, UpdateCx, UpdateProvider};
pub use tool::{call_update_tool, UPDATE_VERBS};

/// The three capabilities the family splits into, by **blast radius**: reading a version is not
/// applying one, and applying one is not holding the backend's credential. They are collapsed onto
/// eight verbs through the cap-alias table in `tool_gate::gate_tool_for` — the only place that
/// collapse is expressible (scope decision 7).
pub use apply::APPLY_CAP;
pub use enrol::CREDENTIAL_CAP;
pub use read::READ_CAP;
