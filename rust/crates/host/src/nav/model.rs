//! The nav record + item types (nav scope, "Data"). A **nav** is a workspace asset, cloned from the
//! `dashboard` shape: a workspace-namespaced `nav:{id}` record holding an ordered `items[]` menu, an
//! owner, and the S4 visibility tier. Sharing to a *team* is a `share` EDGE (reused from `lb_assets`),
//! not a field — so the existing three-gate read check applies unchanged (nav scope, "How it fits").
//!
//! `items` is a typed nested array (queryable, not a JSON blob) — the storage discipline the dashboard
//! scope established. An item is one of four **kinds** plus one level of `group` nesting. The nav is a
//! **lens over existing access, never a grant** — an item carries no caps and cannot widen reach; the
//! resolver strips what the caller can't reach and the server re-checks every verb regardless.

use serde::{Deserialize, Serialize};

/// The table navs live in. Record id is `nav:{id}` (the id is a stable slug, unique per workspace).
pub const TABLE: &str = "nav";

/// The table the per-user active pick lives in — `nav_pref:[ws, user]` (a composite id, member-owned).
/// Deliberately its own table (nav scope: NOT `lb-prefs`, whose axis set is closed to formatting).
pub const PREF_TABLE: &str = "nav_pref";

/// The table the workspace-default pointer lives in — `workspace_nav_default:[ws]` (one row per ws,
/// admin-set). An explicit pointer (the resolved open-question lean) so "the workspace default" is
/// deterministic, not "first/most-recent visibility:workspace nav wins".
pub const DEFAULT_TABLE: &str = "workspace_nav_default";

/// Our nav document version, pinned on [`Nav::schema_version`] at save. Bumped only when the stored
/// document shape changes.
pub const SCHEMA_VERSION: u32 = 1;

/// The largest `items[]` a nav may hold (nav scope, "Resolution cost" / open-question item cap). The
/// host rejects an over-cap save rather than store it unbounded — the resolver stays cheap.
pub const MAX_ITEMS: usize = 100;

/// The largest number of dashboards one `tag-group` entry expands to at resolve time (nav scope: cap
/// tag-group results separately so a broad facet can't blow up the menu). Extra matches are dropped.
pub const MAX_TAG_GROUP: usize = 50;

/// A nav's visibility tier — the S4 asset-sharing tiers (nav scope, "How it fits"; identical to the
/// dashboard tiers, so the same gate-3 read check applies unchanged).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Owner only.
    Private,
    /// Shared to a team via the `share` edge (resolved by team members).
    Team,
    /// Any workspace member with the read cap.
    Workspace,
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility::Private
    }
}

/// One faceted tag query on a `tag-group` item — `{ key, value? }`. A value present means exact
/// (`site:plant-1`); absent means key-only (has any `site`). Mirrors `tags::Facet` on the wire; the
/// resolver maps it to a real `Facet` for `tags.find`. Opaque data (never branched on by the core).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct NavFacet {
    pub key: String,
    /// Exact when present, key-only when absent (nav scope, tag-group entries).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

/// One nav entry. Exactly one of the four **kinds** (`surface` | `dashboard` | `ext` | `tag-group`)
/// or a `group` (one nesting level). All the target-reference fields are opaque data — a `surface`
/// key, a `dashboard:{id}`, an **opaque** ext id (rule 10, never branched on), a facet set — none of
/// which the core interprets beyond the generic gated seams (nav scope, "Four entry kinds").
///
/// The shape is a flat tagged union: `kind` selects which reference fields are meaningful; unused
/// fields default. A `group` carries nested `items` (one level; a nested item's own `items` is
/// ignored — the resolver never recurses past depth 1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NavItem {
    /// `"surface"` | `"dashboard"` | `"ext"` | `"tag-group"` | `"group"`.
    pub kind: String,
    /// The display label. Optional for `surface`/`dashboard`/`ext` (the UI derives one from the
    /// target when empty); required-ish for `tag-group`/`group` (the section header).
    #[serde(default)]
    pub label: String,
    /// `surface`: the opaque core surface key (`"channels"`, `"rules"`, …). Empty otherwise.
    #[serde(default)]
    pub surface: String,
    /// `dashboard`: the `dashboard:{id}` reference (or a bare `{id}`; the resolver accepts both).
    /// Empty otherwise.
    #[serde(default)]
    pub dashboard: String,
    /// `ext`: the **opaque** extension id (rule 10 — never branched on; resolved via `ext.list`).
    /// Empty otherwise.
    #[serde(default)]
    pub ext: String,
    /// `tag-group`: the facets the dynamic entry expands over (resolved via `tags.find`). Empty
    /// otherwise.
    #[serde(default)]
    pub facets: Vec<NavFacet>,
    /// `group`: the nested items (one level of nesting only). Empty otherwise.
    #[serde(default)]
    pub items: Vec<NavItem>,
}

/// A nav record. The persisted menu + sharing metadata (nav scope, "Data").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Nav {
    /// Stable slug, unique per workspace (the record id `nav:{id}`).
    pub id: String,
    pub title: String,
    /// The principal who created it (the private→shared model's anchor).
    pub owner: String,
    #[serde(default)]
    pub visibility: Visibility,
    #[serde(default)]
    pub items: Vec<NavItem>,
    /// Our nav document version — pinned at save.
    #[serde(default, rename = "schemaVersion")]
    pub schema_version: u32,
    pub updated_ts: u64,
    /// Tombstone (soft-delete, §6.8 idempotent). A deleted nav is hidden from `list`/`get`/`resolve`.
    #[serde(default)]
    pub deleted: bool,
}

/// The cheap roster row `nav.list` returns — id/title/visibility/updated_ts, **no `items[]` bodies**
/// (the roster stays cheap; nav scope, "Get / list").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NavSummary {
    pub id: String,
    pub title: String,
    pub visibility: Visibility,
    pub updated_ts: u64,
}

impl From<&Nav> for NavSummary {
    fn from(n: &Nav) -> Self {
        Self {
            id: n.id.clone(),
            title: n.title.clone(),
            visibility: n.visibility,
            updated_ts: n.updated_ts,
        }
    }
}

/// The per-user active pick — `nav_pref:[ws, user]` (nav scope, "A per-user active pick"). A tiny
/// member-owned record naming which nav the member is currently using. Deliberately NOT a `lb-prefs`
/// axis (its axis set is closed to formatting). Absent = no personal pick → fall through to the next
/// resolution tier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct NavPref {
    /// The nav id the member has picked (`nav:{id}`), or empty for "no pick" (a tombstone shape).
    #[serde(default)]
    pub active: String,
    pub updated_ts: u64,
}

/// Which tier `nav.resolve` picked the effective menu from — surfaced so the UI (and the precedence
/// test) can see WHY a given menu was chosen without re-deriving it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResolvedSource {
    /// The member's personal `nav_pref` pick.
    Pick,
    /// The first team-shared nav for one of the member's teams.
    Team,
    /// The workspace-default nav (`workspace_nav_default` pointer).
    WorkspaceDefault,
    /// No nav applied — the caller renders its built-in `SURFACES` fallback.
    Fallback,
}

/// The `nav.resolve` payload — the caller's **effective** menu, already picked, tag-expanded, and
/// cap-stripped (nav scope, "A resolver verb"). `source` names the tier it came from; `nav_id` is the
/// resolved nav (absent on `Fallback`); `items[]` is the resolved menu the UI renders directly. A
/// `Fallback` result carries no items — the UI renders its built-in `SURFACES` (never a blank rail).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedNav {
    pub source: ResolvedSource,
    /// The resolved nav's id (`nav:{id}`), or empty on a `Fallback` (no nav applied).
    #[serde(default)]
    pub nav_id: String,
    #[serde(default)]
    pub title: String,
    /// The resolved, tag-expanded, cap-stripped items. Empty on a `Fallback`.
    pub items: Vec<ResolvedItem>,
}

/// One resolved menu entry — a `NavItem` after tag-expansion + cap-strip. A `tag-group` becomes a
/// `group` of `dashboard` items (its dynamic membership, filtered to what the caller can read); every
/// other kind maps 1:1 (minus any the caller can't reach, which are dropped entirely). The reference
/// fields are the same opaque data as [`NavItem`]; `label` is always populated (derived when the
/// author left it empty) so the UI renders without re-deriving.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedItem {
    pub kind: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub surface: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub dashboard: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ext: String,
    /// Present only on a resolved `group` (from an author `group` OR an expanded `tag-group`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<ResolvedItem>,
}
