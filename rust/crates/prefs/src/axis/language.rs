//! `language` axis — the catalog/locale the user reads in. Stored as a BCP-47 base code string so
//! the mechanism never assumes exactly two languages (prefs scope: "adding `fr` is dropping a
//! catalog file, not a code change"). What is *enabled* on this build is the [`ENABLED`] slice —
//! the Pi-profile CLDR slice the binary compiled in (en/es today). An unset or unknown language
//! falls back to the built-in `en` at resolution, never errors.

/// The languages whose CLDR data + catalogs are compiled into this build. Adding one is a build
/// config change (pull its icu data slice + ship its catalog), not a code edit elsewhere. The
/// renderer maps each to an icu locale; resolution validates against this list.
pub const ENABLED: [&str; 2] = ["en", "es"];

/// The built-in fallback language — the last link in the resolution chain (prefs scope fallback
/// `en, UTC, iso, metric`).
pub const FALLBACK: &str = "en";

/// Is `lang` enabled on this build? An unknown/disabled language resolves to [`FALLBACK`].
pub fn is_enabled(lang: &str) -> bool {
    ENABLED.contains(&lang)
}
