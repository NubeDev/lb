//! The report record + block types (reports scope, "Data"). A **report** is an asset — the
//! `dashboard`/`panel` shape: a workspace-namespaced `report:{id}` record holding an **ordered
//! `blocks[]` array** (the notebook), an owner, the S4 visibility tier, a `brand_id`, and an opaque
//! report-level `toolbar` (range/vars). LWW whole-record save (dashboard `cells[]` precedent) —
//! free undo + free reorder.
//!
//! Three block kinds, one envelope: `markdown` (a body string + `page_break`), `image` (an
//! `asset_id` into the shipped `assets.*` store), and `panel` — **exactly the shipped Cell duality**
//! (a `panel_ref` library panel or an inline spec), so it hydrates/validates through the shipped
//! `hydrate_cells`/`validate_and_strip_refs` seams with no new code.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The table reports live in. Record id is `report:{id}` (a stable slug, unique per workspace).
pub const TABLE: &str = "report";

/// Our report-model document version, pinned on [`Report::schema_version`] at save.
pub const SCHEMA_VERSION: u32 = 1;

/// Soft cap on blocks per report (reports scope Risks: "state the block-count bound ~200 blocks").
/// A save over this is rejected loudly — never silently truncated.
pub const MAX_BLOCKS: usize = 200;

/// A report's visibility tier — the S4 asset-sharing tiers (identical to the panel tiers, so the
/// same gate-3 read check applies unchanged).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Owner only.
    Private,
    /// Shared to a team via the `share` edge (read by team members).
    Team,
    /// Any workspace member with the read cap.
    Workspace,
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility::Private
    }
}

/// One ordered block in a report — a tagged union over the three kinds (`kind` names it). Every
/// non-`kind` field is serde-defaulted so a block of any kind round-trips with only its own fields
/// populated; a `panel` block embeds a full [`crate::dashboard::Cell`] (a panel block IS a Cell —
/// reports scope, "Panel blocks reuse the shipped host functions directly").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Block {
    /// `"markdown"` | `"image"` | `"panel"`.
    pub kind: String,
    /// `markdown`: the body markdown (GFM). Empty otherwise.
    #[serde(default)]
    pub body: String,
    /// `image`: the `asset_id` into the shipped `assets.*` store. Empty otherwise.
    #[serde(default, rename = "assetId")]
    pub asset_id: String,
    /// `image`: an optional caption.
    #[serde(default)]
    pub caption: String,
    /// `image`/`panel`: an opaque width hint (e.g. `"full"` | number). Host-opaque.
    #[serde(default)]
    pub width: Value,
    /// `markdown`: emit a page break after this block (the lazybones page semantics).
    #[serde(default, rename = "pageBreak")]
    pub page_break: bool,
    /// Per-block options (a panel block's pinned range override, etc.). Host-opaque.
    #[serde(default)]
    pub options: Value,
    /// `panel`: the embedded v3 cell — a library `panel_ref` or an inline spec, hydrated/validated
    /// through the shipped seams. Default (empty) on a markdown/image block.
    #[serde(default)]
    pub cell: crate::dashboard::Cell,
}

/// A report record. The persisted blocks + sharing/brand metadata (reports scope, "Data").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Stable slug, unique per workspace (the record id `report:{id}`).
    pub id: String,
    /// The asset name (free to rename; the slug is forever).
    pub title: String,
    /// The principal who created it.
    pub owner: String,
    #[serde(default)]
    pub visibility: Visibility,
    /// The ordered notebook (whole-record LWW; reorder is free).
    #[serde(default)]
    pub blocks: Vec<Block>,
    /// The `brand:{id}` this report renders with (empty = the neutral default).
    #[serde(default, rename = "brandId")]
    pub brand_id: String,
    /// Report-level range/vars — stored **opaquely** (NOT the dashboard `Toolbar` model; the client
    /// owns the typed shape and re-sends it verbatim, the closed-struct discipline).
    #[serde(default)]
    pub toolbar: Value,
    /// OUR report-model document version — pinned at save (`SCHEMA_VERSION`).
    #[serde(default, rename = "schemaVersion")]
    pub schema_version: u32,
    pub updated_ts: u64,
    /// Tombstone (soft-delete, §6.8 idempotent).
    #[serde(default)]
    pub deleted: bool,
}

/// The cheap roster row `report.list` returns — id/title/visibility/updated_ts + a block count (no
/// block bodies).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportSummary {
    pub id: String,
    pub title: String,
    pub visibility: Visibility,
    pub updated_ts: u64,
    #[serde(rename = "blockCount")]
    pub block_count: usize,
}

impl From<&Report> for ReportSummary {
    fn from(r: &Report) -> Self {
        Self {
            id: r.id.clone(),
            title: r.title.clone(),
            visibility: r.visibility,
            updated_ts: r.updated_ts,
            block_count: r.blocks.len(),
        }
    }
}
