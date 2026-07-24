//! The store-mutation error domain. `Denied` stays opaque (an auth signal must not leak whether the
//! table exists); a bad argument surfaces as author feedback; a store fault is an extension error.

use thiserror::Error;

/// The outcome of a `store.write` / `store.delete` that did not succeed.
#[derive(Debug, Error)]
pub enum StoreMutateError {
    /// Gate 1 (workspace) or gate 2 (`store:<table>:write`) failed — opaque, no existence signal.
    #[error("denied")]
    Denied,
    /// A missing/invalid argument (`table`/`id`/`value`) — feedback for the caller, not an auth signal.
    #[error("{0}")]
    BadInput(String),
    /// The table is host-owned (`lb_store::reserved`) — the MCP mutate surface never touches it,
    /// regardless of grants (`store:*:write` does not pierce; ext-store-nodes scope). Deliberately
    /// NOT opaque: the reserved set is a public const, so naming it is author feedback, not a leak.
    #[error("reserved table: {table}")]
    ReservedTable { table: String },
    /// The underlying store rejected the mutation.
    #[error(transparent)]
    Store(#[from] lb_store::StoreError),
}
