//! [`DashboardSummary`] — the CHEAP roster row `dashboard.list` returns.
//!
//! Its own file because it is a distinct caller-visible responsibility from the record: the roster
//! renders and PARTITIONS from this alone, and every field here exists to spare a full
//! `dashboard.get` per row. That is also the rule for adding one — a field belongs here only if the
//! roster would otherwise have to fetch the whole record to paint or filter a row.

use serde::{Deserialize, Serialize};

use super::model::{Dashboard, Visibility};

/// The cheap roster row `list` returns — id/title/visibility/updated_ts, **no cell bodies** (the
/// roster stays cheap; dashboard scope, "Get / list").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub id: String,
    pub title: String,
    /// Roster affordances (dashboard page-settings) — carried on the cheap summary so the switcher
    /// can paint the icon/colour without a full `get`. Additive/defaulted.
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub color: String,
    /// The managing extension's bare id, or empty (ext-managed-dashboards D3). Relayed on the CHEAP
    /// summary so a roster paints the "managed by X" badge — and filters/groups on it — without a
    /// full `dashboard.get` per row. Additive/defaulted; see [`Dashboard::managed_by`].
    #[serde(default, rename = "managedBy")]
    pub managed_by: String,
    /// The record kind, carried on the CHEAP summary because the roster is exactly where the two
    /// kinds are partitioned ([`super::kind`]). Additive/defaulted.
    #[serde(default)]
    pub kind: String,
    pub visibility: Visibility,
    pub updated_ts: u64,
}

impl From<&Dashboard> for DashboardSummary {
    fn from(d: &Dashboard) -> Self {
        Self {
            id: d.id.clone(),
            title: d.title.clone(),
            icon: d.icon.clone(),
            color: d.color.clone(),
            managed_by: d.managed_by.clone(),
            kind: d.kind.clone(),
            visibility: d.visibility,
            updated_ts: d.updated_ts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::KIND_REPORT;

    /// The projection carries exactly the fields a roster needs to PAINT a row (icon/colour) and to
    /// PARTITION it (kind) without a full `dashboard.get`. Moved here with the type itself.
    #[test]
    fn the_summary_carries_what_the_roster_paints_and_partitions_on() {
        let d = Dashboard {
            id: "ops".into(),
            title: "Ops".into(),
            icon: "activity".into(),
            color: "#3b82f6".into(),
            kind: KIND_REPORT.into(),
            ..Dashboard::default()
        };
        let sum = DashboardSummary::from(&d);
        assert_eq!(sum.icon, "activity");
        assert_eq!(sum.color, "#3b82f6");
        assert_eq!(sum.kind, "report");

        // A record that never named a kind projects an EMPTY kind — which reads as "dashboard".
        let plain = DashboardSummary::from(&Dashboard::default());
        assert_eq!(plain.kind, "");
    }
}
