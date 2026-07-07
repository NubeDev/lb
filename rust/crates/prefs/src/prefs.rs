//! The two preference shapes:
//!   - [`Prefs`] — the **stored** record, every axis `Option` so "unset → inherit" is *structural*,
//!     not a sentinel value (prefs scope). One of these per `user_prefs:[ws,user]` and per
//!     `workspace_prefs:[ws]`.
//!   - [`ResolvedPrefs`] — the **folded** result of the resolution chain, every axis decided. This
//!     is what `format.*` reads; it has no `Option`.
//!
//! `unit_overrides` is a CLOSED map `Dimension -> Unit` (prefs scope: keep it a small named enum,
//! never open free text). Serialized as an object keyed by the dimension's wire token.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

use crate::axis::{DateStyle, Dimension, FirstDay, NumberFormat, TimeStyle, Unit, UnitSystem};

/// A stored preference record (user OR workspace-default). Every axis is nullable: `None` means
/// "inherit from the next link in the chain". A patch (`prefs.set`) is the same shape — a present
/// field sets that axis, an absent field leaves it untouched.
// NB: no `Eq` — `ui_theme: Option<serde_json::Value>` is not `Eq` (JSON numbers can be floats).
// `PartialEq` is enough for every caller (tests compare with `assert_eq!`, which uses `PartialEq`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Prefs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_style: Option<DateStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_style: Option<TimeStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_day_of_week: Option<FirstDay>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_format: Option<NumberFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_system: Option<UnitSystem>,
    /// Per-dimension display-unit overrides; a missing dimension inherits the `unit_system` default.
    /// An empty map and `None` are distinct only on the wire — both mean "no overrides here". The
    /// store returns an unset `option<object>` column as JSON `null`, so deserialize null → empty.
    #[serde(
        default,
        deserialize_with = "null_as_empty_map",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub unit_overrides: BTreeMap<Dimension, Unit>,
    /// The member's (or workspace-default's) UI theme, an **opaque** JSON blob owned by the frontend
    /// (`ThemePreference`: mode/preset/radius/custom/imported). Prefs stores and folds it whole —
    /// it never inspects the shape (the theme layer validates it client-side). `None` = inherit the
    /// next link. This is a UI presentation axis riding the prefs record so a theme roams and a
    /// workspace can ship a default; it is not an i18n axis and no `format.*` reads it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_theme: Option<serde_json::Value>,
    /// The workspace **branding** blob (workspace-branding scope): admin-owned workspace identity
    /// (`{ site_name, site_abbr, tagline, login_heading }`). Strings only — image marks live as
    /// assets at reserved ids (`branding:{icon,favicon,logo}`), NOT in this blob (a blob that grew
    /// with each upload would bloat every prefs read). Prefs stores/folds it whole; like `ui_theme`
    /// it is opaque data the frontend validates. The resolved value is the **workspace-default**
    /// link in practice — branding is admin-owned, so a member never writes this axis, but the same
    /// fold machinery carries it for free. `None` = inherit (the shell falls back to its compiled
    /// Lazybones default brand).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_branding: Option<serde_json::Value>,
    /// The member's (or workspace-default's) insight-notification global kill switch
    /// (insights-notify-scope.md). `None` = inherit (the resolved chain defaults to `true` —
    /// notifications ON). `Some(false)` ⇒ the digest reactor skips every delivery for this member's
    /// subscriptions (accounting continues, so re-enabling picks up sane digests). This is a
    /// whole-fold nullable axis (the shipped prefs pattern) — the host reads it once at delivery
    /// time; zero host/gateway plumbing beyond that read. Not an i18n axis; no `format.*` reads it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insight_notifications: Option<bool>,
    /// The member's (or workspace-default's) **default agent persona id** (persona-session #5 — where
    /// #1's `agent.config.active_persona` toggle re-homed). Consulted by the host's `resolve_persona`
    /// when an invoke carries no explicit `persona`: member record → workspace-default record, first
    /// `Some` wins. The id is OPAQUE data (rule 10); a dangling id warns + runs un-narrowed at the
    /// consumer, never here. `None` = inherit; an **empty string clears the axis** (the MERGE-can't-
    /// write-null workaround — the consumer's `filter(|s| !s.is_empty())` treats it as unset). A
    /// whole-fold nullable axis (the `insight_notifications` pattern): not an i18n axis, not in
    /// `ResolvedPrefs`, no `format.*` reads it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_persona: Option<String>,
}

/// Deserialize an `option<object>` column: a present map decodes normally, a stored `null` (the
/// unset column) becomes an empty map rather than a hard error.
fn null_as_empty_map<'de, D>(de: D) -> Result<BTreeMap<Dimension, Unit>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<BTreeMap<Dimension, Unit>>::deserialize(de)?;
    Ok(opt.unwrap_or_default())
}

/// A fully-resolved set of preferences — every axis decided by the resolution chain. The input to
/// every `format.*` call; never `Option`, never inherits further.
// NB: no `Eq` — carries the same non-`Eq` `ui_theme` blob as `Prefs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedPrefs {
    pub language: String,
    pub timezone: String,
    pub date_style: DateStyle,
    pub time_style: TimeStyle,
    pub first_day_of_week: FirstDay,
    pub number_format: NumberFormat,
    pub unit_system: UnitSystem,
    pub unit_overrides: BTreeMap<Dimension, Unit>,
    /// The resolved UI theme blob (member → workspace-default → built-in `None`). Opaque; the theme
    /// layer parses it. `None` when neither the member nor the workspace set one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_theme: Option<serde_json::Value>,
    /// The resolved workspace branding blob (workspace-branding scope). Opaque; the frontend's
    /// `lib/branding` parses it. `None` when no workspace default is set — the shell falls back to
    /// the compiled Lazybones brand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_branding: Option<serde_json::Value>,
}

impl ResolvedPrefs {
    /// The display unit for `dimension`: an explicit override wins, else the `unit_system` default.
    /// This is the `to` unit `format.quantity` converts a canonical value into.
    pub fn display_unit(&self, dimension: Dimension) -> Unit {
        self.unit_overrides
            .get(&dimension)
            .copied()
            .unwrap_or_else(|| self.unit_system.default_unit(dimension))
    }
}
