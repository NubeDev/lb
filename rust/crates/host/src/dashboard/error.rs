//! The dashboard service error. A `Denied` carries no detail (which gate failed, or whether the
//! dashboard exists) — an un-granted or non-member caller leaks nothing about what dashboards exist
//! (dashboard scope, §3.5; mirrors `AssetError`/`IngestError`). `NotFound` only ever reaches a caller
//! who already passed gates 1+2 (so it is not an existence oracle to an outsider).

use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DashboardError {
    /// Authorization failed — workspace isolation, a missing capability, or the gate-3 membership
    /// check (a non-member reading a team-shared dashboard). Opaque by design.
    #[error("denied")]
    Denied,
    /// The write was refused on a **managed** dashboard — one an extension generates
    /// (`managedBy = <bare ext id>`; ext-managed-dashboards scope, Goal 5). Carries the marker so a
    /// client can render "managed by `<id>` — duplicate to edit" instead of a bare denial.
    ///
    /// **Not an existence oracle.** This variant is produced ONLY for a caller who could already
    /// *read* the dashboard (gates 1+2 passed AND gate 3 / `may_read_dashboard` says yes) — such a
    /// caller can see the record and its `managedBy` via `dashboard.get` anyway, so naming the
    /// marker in the refusal reveals nothing new. Every other refused caller gets the opaque
    /// [`DashboardError::Denied`]. Do not widen that rule.
    #[error("denied: managed={0}")]
    ManagedDenied(String),
    /// The dashboard does not exist (or is tombstoned) — reachable only after gates 1+2 pass.
    #[error("not found")]
    NotFound,
    /// The input was not a valid dashboard / arguments for the verb.
    #[error("bad input: {0}")]
    BadInput(String),
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
}
