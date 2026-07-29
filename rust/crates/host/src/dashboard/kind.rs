//! What a dashboard record IS — the `kind` vocabulary and its one predicate.
//!
//! A **report is a dashboard** (reports-as-dashboards scope): the same record, the same grid, the
//! same verbs, laid out on paper. What distinguishes it is this one field.
//!
//! Three properties the rest of the host depends on:
//!
//! - **Empty means [`KIND_DASHBOARD`].** A record written before this field existed round-trips as
//!   empty and must read as an ordinary dashboard, so there is no migration anywhere.
//! - **[`Dashboard::is_report`] is the ONE predicate.** Nothing else compares `kind` to a literal;
//!   scattered `== "report"` checks are how "absent means dashboard" stops being true in one place.
//! - **It is TYPED, and it rides the summary.** An untyped top-level key would be dropped by the
//!   `Dashboard` struct on the first save, and the roster — which is exactly where the two kinds get
//!   partitioned — would need a full `dashboard.get` per row to tell them apart.
//!
//! Unlike `width`, an unknown value is REFUSED at save (`super::save::check_kind`): a bad `width`
//! degrades to the default layout and is visible on screen, whereas a mistyped `kind` drops the
//! record out of both rosters — it saves "successfully" and can then be found nowhere.

use super::model::Dashboard;

/// An ordinary board. Empty means this.
pub const KIND_DASHBOARD: &str = "dashboard";
/// A paper-shaped board `report.export` composes A4 pages from.
pub const KIND_REPORT: &str = "report";

impl Dashboard {
    /// Is this record a report? The one kind predicate.
    pub fn is_report(&self) -> bool {
        self.kind == KIND_REPORT
    }
}
