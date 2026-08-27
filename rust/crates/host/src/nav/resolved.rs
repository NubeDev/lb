//! The **resolved** nav shapes — what `nav.resolve` hands back, as distinct from the stored `nav`
//! record in [`super::model`].
//!
//! A [`ResolvedNav`] is a menu that has already been picked (which tier), tag-expanded, cap-stripped
//! and hidden-stripped; its [`ResolvedItem`]s are [`super::model::NavItem`]s after that pass. They
//! live in their own file because they are the resolver's OUTPUT contract — the wire shape the UI
//! renders — and they change for resolver reasons, never for storage ones.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
    /// The resolved, tag-expanded, cap-stripped, hidden-stripped items. Empty on a `Fallback`.
    pub items: Vec<ResolvedItem>,
    /// The workspace hidden-set ECHO (hide-and-pins scope) — the refs the admin hid, returned so the
    /// UI can subtract them from its built-in `SURFACES`/ext-slot **fallback** too (the one tier the
    /// server cannot strip, because the fallback menu lives client-side). Present on every source,
    /// including `Fallback`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<String>,
    /// The workspace ORDERING echo (`NavHidden::order`) — returned for the same reason as `hidden`:
    /// the built-in `SURFACES`/ext-slot fallback menu lives client-side, so the server cannot apply
    /// an ordering to the one tier it never materializes. Resolved `items` ARE already ordered by
    /// this list; the echo lets the fallback tier apply the identical ordering itself. Present on
    /// every source, including `Fallback`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
    /// The caller's **pinned favorites**, already resolved (cap-, uninstalled-ext-, and
    /// hidden-stripped — hide beats pin), in the member's order. Present on every source, including
    /// `Fallback` — pins render above whichever menu applies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pinned: Vec<ResolvedItem>,
}

/// One resolved menu entry — a `NavItem` after tag-expansion + cap-strip. A `tag-group` becomes a
/// `group` of `dashboard` items (its dynamic membership, filtered to what the caller can read); every
/// other kind maps 1:1 (minus any the caller can't reach, which are dropped entirely). The reference
/// fields are the same opaque data as [`NavItem`]; `label` is always populated (derived when the
/// author left it empty) so the UI renders without re-deriving.
// `Default` so a resolver branch can spell only the fields its kind actually uses and let the rest
// (including any later-added relay field like `nav`) fall out — adding a field to this struct must
// not mean touching every construction site.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ResolvedItem {
    pub kind: String,
    pub label: String,
    /// The author's icon name, echoed through untouched (opaque — the UI maps it, defaulting per
    /// kind when empty or unknown).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub icon: String,
    /// The author's icon color, echoed through untouched (opaque — the UI interprets it, falling back
    /// to its own coloring when empty).
    #[serde(
        default,
        rename = "iconColor",
        skip_serializing_if = "String::is_empty"
    )]
    pub icon_color: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub surface: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub dashboard: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ext: String,
    /// The extension's declared `[[ui.nav]]` destination this item resolves to — the `<navid>`
    /// segment of an `ext:<ext>/<navid>` ref (ext-subref-pins scope). Empty for a whole-extension
    /// item and for every non-ext kind, so the client reduces THIS item back to the same ref it
    /// pinned (`ext:<ext>/<navid>`) rather than to the extension as a whole — which is what makes
    /// the pinned-state highlight land on one row instead of all of that ext's rows. Opaque relay
    /// data; the host matches it as a string and branches on no id (rule 10). Serde-defaulted +
    /// skipped-when-empty, so an old client and a pre-field record both read exactly as today.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub nav: String,
    /// Present only on a resolved `group` (from an author `group`, an expanded `tag-group`, OR an
    /// expanded `template-group`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<ResolvedItem>,
    /// The **resolved variable binding** the UI folds into the href as `?var-<name>=<value>`
    /// (reusable-pages scope): a pinned `dashboard` entry's `vars`, or a template-group child's
    /// `{ <var>: <value> }`. Empty for entries with no binding.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub vars: BTreeMap<String, String>,
    /// The authored [`NavItem::title_template`], relayed verbatim beside `vars` (nav-context-builtins
    /// scope, §G4) — the heading override the client interpolates against the page's `VarScope` at
    /// render. Present on a `template-group` fan-out's CHILDREN too, so each generated instance names
    /// itself; the host expands nothing. `None` when the author pinned none, and `None` in a denied
    /// caller's payload for the trivial reason that a stripped item has no `ResolvedItem` at all.
    /// Serde-defaulted + skipped-when-absent, so an old client and a pre-field record read as today.
    #[serde(
        default,
        rename = "titleTemplate",
        skip_serializing_if = "Option::is_none"
    )]
    pub title_template: Option<String>,
    /// The authored [`NavItem::home`], relayed verbatim (nav-home scope) so the client can land on
    /// the marked entry rather than guessing from position. Carried on the RESOLVED item because the
    /// menu the client sees is cap-stripped and tag-expanded: a home the caller may not reach never
    /// arrives, which is exactly the fallback signal the client needs. Serde-defaulted + skipped when
    /// false, so an old client and a pre-field record read as today.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub home: bool,
}
