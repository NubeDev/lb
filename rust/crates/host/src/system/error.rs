//! The error type for the system-observability verbs. Like every host read service, a gate failure
//! collapses to an opaque `Denied` (no existence/detail signal, §5); a store fault surfaces as
//! `Store`. Both `system.overview` and `system.topology` read only — there is no mutation variant.

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SystemError {
    /// The workspace-first / `mcp:system.*:call` gate refused — opaque on purpose.
    #[error("denied")]
    Denied,
    /// A read against the embedded store failed (every raw subsystem read surfaces here).
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
