//! CLDR plural-category selection for the enabled languages (i18n-catalogs scope, "Plural rules —
//! hand-encoded, en/es, per CLDR 44"). Deliberately NOT full CLDR: en and es both have exactly the
//! two cardinal categories `one` (n == 1) and `other` (everything else). The first language that
//! needs more categories (`pl`, `ar`) is the trigger to swap in icu4x's CLDR plural engine — flagged
//! in the scope, NOT hand-extended here.
//!
//! Exact-value arms (`=0`, `=1`) are matched by the renderer *before* it consults this function, so
//! this only decides between the keyword categories. Pure: no I/O, no locale data beyond the two
//! hand-encoded rules.

/// The CLDR cardinal plural categories the subset supports. `Other` is the mandatory fallback arm;
/// `One` is the singular. (Additional CLDR categories — `zero`/`two`/`few`/`many` — are out of the
/// pinned subset until the icu4x swap.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluralCategory {
    One,
    Other,
}

impl PluralCategory {
    /// The catalog keyword this category is authored under (`one{…}` / `other{…}`).
    pub fn keyword(self) -> &'static str {
        match self {
            PluralCategory::One => "one",
            PluralCategory::Other => "other",
        }
    }
}

/// Select the cardinal plural category for `n` in `lang` (CLDR 44). en/es share the rule
/// `n == 1 → one, else other`; an unknown/disabled language uses the same two-category rule (the
/// fallback language is `en`, which has it). `n` is the integer plural count — the `#` value.
pub fn category(lang: &str, n: i64) -> PluralCategory {
    // en/es (and the `en` fallback) — CLDR 44 cardinal: one iff n == 1.
    let _ = lang; // both enabled languages use the same rule; kept for the icu4x swap seam.
    if n == 1 {
        PluralCategory::One
    } else {
        PluralCategory::Other
    }
}
