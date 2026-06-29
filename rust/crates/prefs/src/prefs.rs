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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedPrefs {
    pub language: String,
    pub timezone: String,
    pub date_style: DateStyle,
    pub time_style: TimeStyle,
    pub first_day_of_week: FirstDay,
    pub number_format: NumberFormat,
    pub unit_system: UnitSystem,
    pub unit_overrides: BTreeMap<Dimension, Unit>,
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
