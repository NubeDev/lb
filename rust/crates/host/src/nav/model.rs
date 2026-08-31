//! The nav record + item types (nav scope, "Data"). A **nav** is a workspace asset, cloned from the
//! `dashboard` shape: a workspace-namespaced `nav:{id}` record holding an ordered `items[]` menu, an
//! owner, and the S4 visibility tier. Sharing to a *team* is a `share` EDGE (reused from `lb_assets`),
//! not a field — so the existing three-gate read check applies unchanged (nav scope, "How it fits").
//!
//! `items` is a typed nested array (queryable, not a JSON blob) — the storage discipline the dashboard
//! scope established. An item is one of four **kinds** plus `group` nesting (recursive, capped at
//! [`MAX_GROUP_DEPTH`]; nested-nav scope). The nav is a
//! **lens over existing access, never a grant** — an item carries no caps and cannot widen reach; the
//! resolver strips what the caller can't reach and the server re-checks every verb regardless.

use std::collections::BTreeMap;

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

/// The table the workspace hidden-set lives in — `nav_hidden:[ws]` (one row per ws, admin-set;
/// hide-and-pins scope). A subtractive curation record applied by the resolver at EVERY tier
/// (including the built-in `SURFACES` fallback, which the UI subtracts client-side from the
/// `ResolvedNav::hidden` echo). Hiding never blocks a route — declutter only, never authz.
pub const HIDDEN_TABLE: &str = "nav_hidden";

/// Our nav document version, pinned on [`Nav::schema_version`] at save. Bumped only when the stored
/// document shape changes.
pub const SCHEMA_VERSION: u32 = 1;

/// The largest `items[]` a nav may hold (nav scope, "Resolution cost" / open-question item cap). The
/// host rejects an over-cap save rather than store it unbounded — the resolver stays cheap. Counts
/// EVERY node at EVERY depth (groups included); fires independently of [`MAX_GROUP_DEPTH`].
pub const MAX_ITEMS: usize = 100;

/// The deepest a `group` may nest (nested-nav scope). The top-level `items[]` is depth 1; a `group` at
/// depth 5 may hold leaves but no further `group`. Sourced from ONE place (re-exported on the lib API
/// as `NAV_MAX_GROUP_DEPTH`) so a consumer UI can echo the limit rather than hardcode `5`. `nav.save`
/// rejects an over-cap nesting with `BadInput` (never a silent flatten/truncate), independently of the
/// [`MAX_ITEMS`] node cap.
pub const MAX_GROUP_DEPTH: usize = 5;

/// The largest number of dashboards one `tag-group` entry expands to at resolve time (nav scope: cap
/// tag-group results separately so a broad facet can't blow up the menu). Extra matches are dropped.
pub const MAX_TAG_GROUP: usize = 50;

/// Cap on an item's `icon` name length (an opaque UI icon key — anything longer is garbage, not a
/// name; rejected `BadInput` at save like the other bounds).
pub const MAX_ICON_LEN: usize = 64;

/// Cap on an item's `icon_color` length (an opaque UI color token — a `#rrggbb` hex or a short
/// palette name; anything longer is garbage, not a color). Rejected `BadInput` at save like
/// [`MAX_ICON_LEN`]. The core never parses the value — it is opaque data the UI interprets.
pub const MAX_ICON_COLOR_LEN: usize = 32;

/// Cap on an item's `title_template` (nav-context-builtins scope) — the heading-override template.
/// Re-exported from `lb_ext_loader` so the manifest path and the nav-builder write path cap the SAME
/// field with the SAME number (rule 10: one field, one validator, one bound, whichever door it came
/// through). Rejected `BadInput` at save like [`MAX_ICON_LEN`].
pub use lb_ext_loader::NAV_MAX_TITLE_TEMPLATE as MAX_TITLE_TEMPLATE;

/// The largest hidden-set `nav.hidden.set` accepts (hide-and-pins scope, "Bounds"). Rejected over-cap
/// (`BadInput`), never silently truncated.
pub const MAX_HIDDEN: usize = 200;

/// The most pins one member may hold (`nav_pref.pinned`; hide-and-pins scope, "Bounds"). Rejected
/// over-cap (`BadInput`), never silently truncated.
pub const MAX_PINNED: usize = 50;

/// The largest ordering `nav.hidden.set` accepts (`NavHidden::order`). Sized like [`MAX_HIDDEN`] —
/// an ordering names the same ref population a hidden-set does, so one bound covers both. Rejected
/// over-cap (`BadInput`), never silently truncated.
pub const MAX_ORDER: usize = 200;

/// A nav's visibility tier — the S4 asset-sharing tiers (nav scope, "How it fits"; identical to the
/// dashboard tiers, so the same gate-3 read check applies unchanged).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Visibility {
    /// Owner only.
    #[default]
    Private,
    /// Shared to a team via the `share` edge (resolved by team members).
    Team,
    /// Any workspace member with the read cap.
    Workspace,
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
/// or a `group` (recursive nesting, capped at [`MAX_GROUP_DEPTH`]). All the target-reference fields are
/// opaque data — a `surface` key, a `dashboard:{id}`, an **opaque** ext id (rule 10, never branched
/// on), a facet set — none of which the core interprets beyond the generic gated seams (nav scope,
/// "Four entry kinds").
///
/// The shape is a flat tagged union: `kind` selects which reference fields are meaningful; unused
/// fields default. A `group` carries nested `items` — itself possibly holding further `group`s, up to
/// [`MAX_GROUP_DEPTH`] deep (top-level list = depth 1). Leaf kinds may appear at any depth.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct NavItem {
    /// `"surface"` | `"dashboard"` | `"ext"` | `"tag-group"` | `"template-group"` | `"group"`.
    pub kind: String,
    /// The display label. Optional for `surface`/`dashboard`/`ext` (the UI derives one from the
    /// target when empty); required-ish for `tag-group`/`group` (the section header).
    #[serde(default)]
    pub label: String,
    /// An optional author-chosen display icon **name** — opaque data the UI maps to its own icon set
    /// (an unknown name falls back to the kind's default; the core never interprets it). Bounded by
    /// [`MAX_ICON_LEN`] at save. Meaningful on any kind; empty = the UI's per-kind default.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub icon: String,
    /// An optional author-chosen icon **color** — opaque data the UI interprets (a `#rrggbb` hex or a
    /// short palette token; the core never parses it). Bounded by [`MAX_ICON_COLOR_LEN`] at save.
    /// Meaningful on any kind; empty = the UI's own default coloring for that item.
    #[serde(
        default,
        rename = "iconColor",
        skip_serializing_if = "String::is_empty"
    )]
    pub icon_color: String,
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
    /// `group`: the nested items — recursive, capped at [`MAX_GROUP_DEPTH`] (nested-nav scope). Empty
    /// otherwise.
    #[serde(default)]
    pub items: Vec<NavItem>,
    /// `dashboard` / `template-group`: an optional **pinned variable binding** (reusable-pages scope)
    /// rendered into the link as `?var-<name>=<value>` — a curated, durable, named page instance
    /// ("Plant-1 Overview"). Opaque data; the resolver carries it through to `ResolvedItem::vars` and
    /// the UI folds it into the href. A `BTreeMap` for deterministic order (round-trip + `PartialEq`).
    /// Empty for the other kinds.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub vars: BTreeMap<String, String>,
    /// `template-group`: the template's **parameter** (a `Variable` name) this entry binds — one page
    /// instance per enumerated option value (`?var-<var>=<value>`). Empty otherwise.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub var: String,
    /// `template-group`: an **option-source tool** (the `Variable.query` `{tool,args}` shape) — the
    /// general fan-out source, an alternative to `facets`. Empty otherwise; when set, the resolver
    /// re-enters the generic dispatcher under the caller's caps to enumerate values (the lens).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tool: String,
    /// `template-group`: the option-source tool's args (opaque; re-checked per call). `Null` otherwise.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub args: serde_json::Value,
    /// An OPTIONAL **heading override** this item pins on the board it opens (nav-context-builtins
    /// scope, §G4) — a TEMPLATE STRING the host stores and relays verbatim, expanding nothing (the
    /// `Action.args_template` posture). The shell interpolates it at render against the page's
    /// `VarScope` — this item's `vars` (or, on a `template-group`, the fanned-out `var` binding) plus
    /// the `__nav.*` / `__page.*` built-ins — and shows it in place of the board's stored `heading`.
    /// That is what lets ONE template board say which thing the viewer is looking at.
    ///
    /// Same field, same [`MAX_TITLE_TEMPLATE`] cap and the same validator as the extension manifest's
    /// `[[ui.nav]] title_template` (rule 10 — the ext seam is not the privileged path; an admin
    /// authoring in the nav builder is checked identically). Meaningful on `dashboard` /
    /// `template-group`; carried opaquely on any kind. `None` on every record written before this
    /// field — additive, so [`SCHEMA_VERSION`] is unchanged and no migration exists.
    #[serde(
        default,
        rename = "titleTemplate",
        skip_serializing_if = "Option::is_none"
    )]
    pub title_template: Option<String>,
    /// The author's **home** marker (nav-home scope): the ONE entry a caller narrowed to this menu
    /// lands on after login, instead of the client's positional guess.
    ///
    /// Landing already followed the menu, but by POSITION — whatever destination came first. That is
    /// invisible in the record and moves the moment someone reorders the rail: a site dragged above
    /// the portfolio silently becomes everyone's home. Saying it here makes it an authored fact the
    /// order cannot disturb.
    ///
    /// Meaningful on any kind that HAS a destination (`surface` / `dashboard` / `ext`, and a `group`
    /// carrying its own `dashboard`); carried opaquely on the rest. At most one per nav, enforced at
    /// save. The client still falls back to its positional pick when a nav marks none, and when the
    /// marked item was stripped from THAT caller's menu by their caps — a home nobody can reach is
    /// not a lockout, it is simply absent (nav's degrade-open posture).
    ///
    /// Additive: serde-defaulted and skipped when false, so every record written before this field
    /// reads as `false` and [`SCHEMA_VERSION`] is unchanged — the `title_template` precedent.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub home: bool,
    /// The author's **footer** marker (nav-footer scope): this TOP-LEVEL entry renders at the END of
    /// the menu's axis (the bottom of a rail, the trailing end of a top bar) instead of in the tree.
    /// Legal only at depth 0 (a footer nested in a folder has no meaning), enforced at save.
    ///
    /// Additive: serde-defaulted and skipped when false, so every record written before this field
    /// reads as `false` and [`SCHEMA_VERSION`] is unchanged — the `home` precedent.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub footer: bool,
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
    /// The member's **pinned favorites** (hide-and-pins scope) — an ORDERED list of item refs in the
    /// shared ref grammar: a bare surface key (`"rules"`), `ext:<id>` (opaque, rule 10), or
    /// `dashboard:<id>`. Resolved server-side into `ResolvedNav::pinned` (cap- and hidden-stripped);
    /// a stale ref strips silently at resolve WITHOUT mutating this record, so restores are free.
    /// Additive field — a pre-pins record deserializes with no pins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pinned: Vec<String>,
    /// **Force the built-in sidebar** (no-lockout scope) — the escape-hatch override, in its OWN
    /// slot, deliberately DECOUPLED from `active`. When true, the resolver skips the pick and the
    /// team/default tiers and returns the built-in `SURFACES` fallback. The shell toggles THIS field
    /// ("Show all pages" / "Use my menu") and never writes `active`, so the member's real pick
    /// survives the round-trip losslessly — an admin's curated nav is restored on "Use my menu"
    /// because it was never deleted (the old single-slot `active == "__builtin__"` sentinel
    /// destroyed it; see `pick_nav` for the legacy convergence). Additive field — a pre-split record
    /// deserializes with it `false` (serde default).
    #[serde(default)]
    pub force_builtin: bool,
    pub updated_ts: u64,
}

/// The workspace hidden-set record (`nav_hidden:[ws]`; hide-and-pins scope) — the admin's one
/// subtractive curation lever. `hidden` holds item refs in the same grammar as [`NavPref::pinned`]
/// (bare surface key | `ext:<id>` | `dashboard:<id>`), each treated as OPAQUE data (rule 10). Hiding
/// is a lens like the nav itself: a hidden page a caller is permitted to reach still loads by deep
/// link — the resolver only stops LISTING it (menu + pins; hide beats pin).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct NavHidden {
    #[serde(default)]
    pub hidden: Vec<String>,
    /// The workspace's ORDERING over the same opaque ref grammar — a partial, positional preference,
    /// not a membership list. A ref named here sorts to its position; every ref NOT named keeps its
    /// natural (caller-side) order after the named ones, so an ordering never hides an item and a
    /// stale ref (an uninstalled ext, a deleted dashboard) is inert rather than destructive. Group
    /// headings order by their `group:<Label>` ref alongside the items they contain. Additive field —
    /// a pre-ordering record deserializes with it empty (serde default), meaning "natural order".
    #[serde(default)]
    pub order: Vec<String>,
    pub updated_ts: u64,
}
