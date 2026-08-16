//! The **host-authored ext nav boards** record — the persisted map binding host dashboards into an
//! extension's sidebar section (host-authored-ext-nav-boards scope).
//!
//! One workspace record, `nav_ext_boards:[ws]`, exactly the scope + lifetime of `nav_hidden`. Its
//! keys are **opaque slot refs** (`ext:<id>` for a section root, `ext:<id>/<navid>` for a declared
//! item) and its values are **opaque dashboard refs** — the core never interprets either beyond the
//! generic ref grammar (rule 10: no extension is named in this path, and nothing here is branched on
//! an id). The shell merges these rows into the section it already renders, so an operator can place
//! a board under an extension WITHOUT the extension knowing, cooperating, or republishing.
//!
//! **A row is a lens, never a grant.** It contributes no capability of its own; the board's own
//! viewer gate is the authority, and the row follows the section's reach.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The one host-authored-boards record per workspace (`nav_ext_boards:[ws]`).
pub const EXT_BOARDS_TABLE: &str = "nav_ext_boards";

/// The largest number of SLOTS one workspace may bind boards into. A slot is one `ext:<id>` or
/// `ext:<id>/<navid>` key; over-cap is `BadInput`, never a silent truncation.
pub const MAX_EXT_BOARD_SLOTS: usize = 100;

/// The largest number of rows ONE slot may hold — a sidebar section, not a directory.
pub const MAX_EXT_BOARD_ROWS: usize = 50;

/// The largest number of rows the whole record may hold across every slot. Bounds the resolve cost
/// independently of how the rows are distributed (100 slots × 50 rows would otherwise be 5000).
pub const MAX_EXT_BOARD_TOTAL: usize = 500;

/// Cap on a row's `id` — the ref segment (`ext:<id>/<navid>/<row.id>`), a slug not a sentence.
pub const MAX_EXT_BOARD_ID_LEN: usize = 64;

/// Cap on a row's `label` — literal display text an admin typed (NOT an i18n key).
pub const MAX_EXT_BOARD_LABEL_LEN: usize = 200;

/// Cap on a row's pinned-variable binding, mirroring the bound the nav builder puts on `NavItem`.
pub const MAX_EXT_BOARD_VARS: usize = 20;

/// One host-authored board row inside an extension's nav section. Renders through the SAME path a
/// published `dashboard`-carrying ext child does — same `dashboard:{id}` + `vars` grammar, same
/// stock glyph, same `?var-` folding — so nothing about it is a second render path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ExtBoardRow {
    /// Stable slug, unique within its slot — the last segment of the row's nav ref. Opaque.
    pub id: String,
    /// The `dashboard:{id}` reference (a bare `{id}` is accepted too, like `NavItem::dashboard`).
    /// Opaque data: the host never resolves it here, and a dangling ref renders rather than hides
    /// (the viewer's own not-found state is the honest answer — silence would hide a config error).
    pub dashboard: String,
    /// The display label — literal text a host author typed, not an i18n key.
    #[serde(default)]
    pub label: String,
    /// An optional icon NAME, opaque to the core (the UI maps it to its own set and falls back to
    /// its dashboard glyph for an unknown one). Empty = the UI's default.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub icon: String,
    /// An optional icon COLOR token, opaque to the core (a `#rrggbb` or a palette name).
    #[serde(
        default,
        rename = "iconColor",
        skip_serializing_if = "String::is_empty"
    )]
    pub icon_color: String,
    /// An optional pinned variable binding, folded by the shell into `?var-<name>=<value>` — the
    /// same `Record<string,string>` grammar `NavItem::vars` already uses. A `BTreeMap` so the
    /// round-trip is order-deterministic.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub vars: BTreeMap<String, String>,
}

/// The workspace's host-authored ext board rows, keyed by opaque slot ref. An ABSENT record is the
/// empty map — the feature is additive and inert until an admin uses it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ExtNavBoards {
    /// Slot ref (`ext:<id>` | `ext:<id>/<navid>`) → the ordered rows bound into it. Stored order IS
    /// render order; the shell appends them AFTER the extension's own published children.
    #[serde(default)]
    pub slots: BTreeMap<String, Vec<ExtBoardRow>>,
    /// The logical time of the last full-set write (LWW).
    pub updated_ts: u64,
}
