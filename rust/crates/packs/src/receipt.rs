//! The receipt shape — what a pack applied, to which workspace, at what version, with what outcome
//! per object. Pure: this file owns the record, never its persistence.
//!
//! Receipts are FIRST-CLASS in core (pack-core-scope §"Receipts as records"): they live in an
//! internal, caps-walled table read by `pack.list`/`pack.get`, NOT through the public `store.*`
//! verbs. The prototype's `pack_receipts`-via-`store.write` convention — and the whole
//! `SELECT data FROM …` envelope workaround it needed — dies with this port. A reader gets a typed
//! record; nobody has to know a store quirk to read their own apply history.

use serde::{Deserialize, Serialize};

use crate::bundle::Pack;
use crate::plan::PlannedObject;

/// One object's fate in an apply.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ObjectReceipt {
    /// The [`crate::plan::Kind`] string.
    pub kind: String,
    /// The stable id — never a display name; drift detection depends on it.
    pub id: String,
    pub checksum: String,
    /// `"applied"`, `"denied"`, or `"failed"` — the partial-apply signal the recovery path reads.
    pub outcome: String,
}

/// The record of one pack applied to one workspace. One receipt per pack per workspace.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Receipt {
    pub pack: String,
    /// The manifest title, denormalized so a roster read needs no bundle.
    pub title: String,
    pub version: u32,
    pub manifest_checksum: String,
    /// Logical apply timestamp — caller-injected, never a wall clock below the seam.
    pub applied_ts: u64,
    /// The manifest as applied, kept so a reader can render the pack (entities, insight grammar,
    /// agent context path) without re-sending the bundle. `pack.list` strips it; `pack.get` keeps it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<crate::manifest::Manifest>,
    pub objects: Vec<ObjectReceipt>,
}

impl Receipt {
    /// Build a receipt from the plan and the parallel per-object outcomes.
    pub fn from_outcomes(
        pack: &Pack,
        manifest_checksum: String,
        applied_ts: u64,
        plan: &[PlannedObject],
        outcomes: &[String],
    ) -> Receipt {
        Receipt {
            pack: pack.manifest.pack.clone(),
            title: pack.manifest.title.clone(),
            version: pack.manifest.version,
            manifest_checksum,
            applied_ts,
            manifest: Some(pack.manifest.clone()),
            objects: plan
                .iter()
                .zip(outcomes)
                .map(|(o, outcome)| ObjectReceipt {
                    kind: o.kind.as_str().to_string(),
                    id: o.id.clone(),
                    checksum: o.checksum.clone(),
                    outcome: outcome.clone(),
                })
                .collect(),
        }
    }

    /// True when every object applied cleanly — the difference between a done apply and a partial
    /// the caller must recover from.
    pub fn is_complete(&self) -> bool {
        self.objects.iter().all(|o| o.outcome == "applied")
    }
}
