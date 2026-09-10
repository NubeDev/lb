//! `PrefsAxis` — the closed set of clearable preference axes, and the SurrealDB merge fragment that
//! clears them.
//!
//! A patch ([`Prefs`](crate::Prefs)) can only ever *set* an axis: every field is
//! `skip_serializing_if = "Option::is_none"`, so an absent axis means "leave as stored" and there is
//! no representable value meaning "unset this". That made an axis a one-way door — once a member
//! stored one, it shadowed the workspace default forever (the whole-fold axes `ui_theme`/
//! `ui_branding` most visibly, since they fold all-or-nothing).
//!
//! This is the other half of the write vocabulary: an explicit `clear` list travelling alongside the
//! patch. `UPSERT ... MERGE` with an explicit JSON `null` DOES clear a column (verified against a
//! real `mem://` store — the long-standing "MERGE can't write null" belief in this codebase was
//! wrong, which is why `agent_persona` grew its `""`-means-unset workaround). Naming the axes rather
//! than accepting nulls inside the patch keeps the opaque blobs (`ui_theme`, `ui_branding`) opaque:
//! prefs still never inspects their shape, it only ever drops the whole column.
//!
//! One responsibility: name a clearable axis and render its clearing merge object.

use serde::{Deserialize, Serialize};

/// A preference axis that a `clear` list may name. Closed set — the wire form is the snake_case
/// column name, identical to the `Prefs` field it clears, so a caller never has to learn a second
/// vocabulary. Unknown names are rejected at deserialization (serde `deny_unknown_fields` behaviour
/// for enums), so a typo is a loud error rather than a silent no-op.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefsAxis {
    Language,
    Timezone,
    DateStyle,
    TimeStyle,
    FirstDayOfWeek,
    NumberFormat,
    UnitSystem,
    UnitOverrides,
    UiTheme,
    UiBranding,
    InsightNotifications,
    AgentPersona,
    PushMuted,
}

impl PrefsAxis {
    /// The stored column name — the serde field name on `Prefs`, so the two can never drift.
    pub fn column(self) -> &'static str {
        match self {
            Self::Language => "language",
            Self::Timezone => "timezone",
            Self::DateStyle => "date_style",
            Self::TimeStyle => "time_style",
            Self::FirstDayOfWeek => "first_day_of_week",
            Self::NumberFormat => "number_format",
            Self::UnitSystem => "unit_system",
            Self::UnitOverrides => "unit_overrides",
            Self::UiTheme => "ui_theme",
            Self::UiBranding => "ui_branding",
            Self::InsightNotifications => "insight_notifications",
            Self::AgentPersona => "agent_persona",
            Self::PushMuted => "push_muted",
        }
    }
}

/// Render `clear` as merge-object entries, each column mapped to JSON `null`. Applied on top of the
/// patch object, so an axis named in BOTH is cleared (clear wins — the caller asked for the axis to
/// inherit, and honouring the set would silently ignore half the request).
/// The `SET <col> = NONE, …` clause that clears `axes`, or `None` when there is nothing to clear.
///
/// Clearing used to ride in on the MERGE object as JSON `null`. SurrealDB separates **NULL** (a
/// written empty) from **NONE** (no value), and the axis columns are declared `TYPE option<...>`,
/// which admits NONE but not NULL. SurrealDB 3 enforces that on write: every clear failed with
/// "Couldn't coerce value for field `ui_branding`: Expected `none | object` but found `NULL`".
///
/// JSON has no way to say NONE, so a clear cannot travel inside the merge object at all — it has to
/// be SurrealQL. The column names come from [`PrefsAxis::column`], a closed enum returning
/// `&'static str`, so nothing caller-supplied is ever spliced here.
pub(crate) fn clear_set_clause(axes: &[PrefsAxis]) -> Option<String> {
    if axes.is_empty() {
        return None;
    }
    let mut cols: Vec<&'static str> = axes.iter().map(|a| a.column()).collect();
    cols.sort_unstable();
    cols.dedup();
    Some(
        cols.iter()
            .map(|c| format!("{c} = NONE"))
            .collect::<Vec<_>>()
            .join(", "),
    )
}
