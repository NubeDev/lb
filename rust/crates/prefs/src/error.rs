//! The pure-layer error of `lb-prefs` — conversion, formatting and parse failures that have nothing
//! to do with authorization or the store. The host wraps these (and store errors) at its boundary;
//! a capability denial is opaque and lives in the host, never here.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PrefsError {
    /// A unit token outside the closed [`crate::Unit`] set — never guessed, always a hard error
    /// (prefs scope: "an unknown unit is a hard error, not a passthrough").
    #[error("unknown unit: {0}")]
    UnknownUnit(String),

    /// A convert/format.quantity whose `from` and `to` units measure different dimensions — the
    /// classic temperature→speed mistake. Rejected structurally (the units report different
    /// `Dimension`s), never computed.
    #[error("cross-dimension conversion: {from} ({from_dim}) -> {to} ({to_dim})")]
    CrossDimension {
        from: &'static str,
        from_dim: &'static str,
        to: &'static str,
        to_dim: &'static str,
    },

    /// An instant (epoch milliseconds) that could not be interpreted as a valid UTC datetime, or a
    /// timezone id outside the compiled tz database.
    #[error("bad instant or timezone: {0}")]
    BadInstant(String),
}
