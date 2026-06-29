//! The viz resolver error (viz transformations scope). `Denied` is opaque — an un-granted
//! `viz.query` caller leaks nothing (mirrors `DashboardError`/`AssetError`). A denied *target* inside
//! the resolver does NOT surface as `Denied` for the whole call: it degrades to an honest empty frame
//! (the no-bypass / no-fabrication rule) — only a missing `mcp:viz.query:call` or a malformed panel
//! reaches the caller as an error.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum VizError {
    /// `mcp:viz.query:call` (or workspace) denied — opaque.
    #[error("denied")]
    Denied,
    /// The panel spec / args were malformed.
    #[error("bad input: {0}")]
    BadInput(String),
}
