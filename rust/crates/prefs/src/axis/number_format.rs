//! `number_format` axis — the decimal/grouping convention, its OWN axis (prefs resolved decision:
//! "a user can have Spanish text with English-style `.` decimals"). Seeded by the base locale but
//! independently overridable.

use serde::{Deserialize, Serialize};

/// Decimal + grouping separators. `dot_comma` = `1,234.56` (en), `comma_dot` = `1.234,56` (es/eu),
/// `space_comma` = `1 234,56` (fr-style). Closed set; the renderer maps it to an icu locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumberFormat {
    /// `1,234.56` — group `,`, decimal `.` (en-US).
    DotComma,
    /// `1.234,56` — group `.`, decimal `,` (es-ES / de-DE).
    CommaDot,
    /// `1 234,56` — group thin-space, decimal `,` (fr-FR).
    SpaceComma,
}

impl NumberFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            NumberFormat::DotComma => "dot_comma",
            NumberFormat::CommaDot => "comma_dot",
            NumberFormat::SpaceComma => "space_comma",
        }
    }

    /// The grouping separator string this convention uses.
    pub fn group_sep(&self) -> &'static str {
        match self {
            NumberFormat::DotComma => ",",
            NumberFormat::CommaDot => ".",
            NumberFormat::SpaceComma => "\u{202f}", // narrow no-break space
        }
    }

    /// The decimal separator string this convention uses.
    pub fn decimal_sep(&self) -> &'static str {
        match self {
            NumberFormat::DotComma => ".",
            NumberFormat::CommaDot | NumberFormat::SpaceComma => ",",
        }
    }

    pub const ALL: [NumberFormat; 3] = [
        NumberFormat::DotComma,
        NumberFormat::CommaDot,
        NumberFormat::SpaceComma,
    ];
}
