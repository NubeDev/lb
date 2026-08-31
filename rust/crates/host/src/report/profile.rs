//! [`ExportProfile`] — a NAMED set of the options an export already takes, stored on the board it
//! belongs to (report-pagination-and-export-options scope).
//!
//! **This reverses the scope's "no stored export profiles" non-goal, for a mechanical reason.** That
//! non-goal assumed the client could keep its own profiles, and on this record it cannot:
//! [`Dashboard`](crate::Dashboard) has no `#[serde(flatten)]` catch-all, so it DROPS unknown
//! top-level keys on save. A profile written by the client round-trips to nothing on the very next
//! layout save — a control that appears to work and silently forgets, which is exactly what `heading`,
//! `reportIds`, `width` and `compact` each had to be typed to avoid. "State without a reader" is the
//! wrong worry here: the reader is the client, as it is for every one of those fields.
//!
//! It reuses [`ExportOptions`] verbatim and deliberately. A profile IS a named set of the options
//! `report.export` already accepts, so there is exactly ONE option vocabulary — the same argument the
//! two export doors make: a second, parallel spelling of "what a PDF should look like" could only
//! drift from this one.
//!
//! **The host does not read a profile at export time.** Nothing here resolves a profile id, and
//! `report.export` takes no profile argument: the client picks a profile and sends that profile's
//! `options` on the export call. This type is storage and serde, nothing more — do not go looking for
//! the consumer in this crate.

use serde::{Deserialize, Serialize};

use super::options::ExportOptions;

/// One stored export profile: a stable `id`, a human `name`, and the [`ExportOptions`] it stands for.
/// Every field serde-defaulted, so a half-written profile deserializes rather than failing the whole
/// board's read.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ExportProfile {
    /// Stable, client-authored id — what the export dialog remembers as the current pick. Opaque to
    /// the host: never resolved, never validated, never made unique here.
    pub id: String,
    /// The label the picker shows.
    pub name: String,
    /// The options this profile stands for — the identical vocabulary `report.export` accepts.
    pub options: ExportOptions,
}
