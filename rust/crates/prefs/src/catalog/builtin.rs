//! The built-in en/es catalogs, compiled into the host via `include_str!` (i18n-catalogs scope:
//! "MF1 text asset files … compiled into the host … the single source the client bundle is generated
//! from"). This module is the *source of truth*: the `gen_catalog` bin renders the client TS from
//! exactly these parsed maps, and the fallback chain (language → `en` → key) lives here.
//!
//! File format (one responsibility: a flat key→message text asset):
//!   - a leading `catalog-version: <n>` header line (a human stamp echoed in responses),
//!   - `#`-prefixed comment lines (and blank lines) ignored,
//!   - one `key = <MF1 message>` per line — split on the FIRST `=` (a key never contains `=`; the
//!     message may, e.g. a plural `=0` arm, so only the first split matters).
//! Keys are flat dotted strings (`alert.threshold_crossed`), never nested.

use std::collections::BTreeMap;

/// The raw en catalog text — the source both the host and (via `gen_catalog`) the client read.
pub const EN_MF: &str = include_str!("builtin/en.mf");
/// The raw es catalog text.
pub const ES_MF: &str = include_str!("builtin/es.mf");

/// A parsed built-in catalog: the version stamp + the flat key→message map (message text, unparsed
/// — parsing to the MF1 AST happens at render time / lint time).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinCatalog {
    pub version: String,
    pub messages: BTreeMap<String, String>,
}

/// Parse a `.mf` asset's text into a [`BuiltinCatalog`]. Panics only on a malformed *built-in* (a
/// compile-time asset we control — a broken builtin is a build bug, surfaced loudly by the loader
/// test), never on tenant data.
pub fn parse_builtin(src: &str) -> BuiltinCatalog {
    let mut version = String::from("0");
    let mut messages = BTreeMap::new();
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("catalog-version:") {
            version = rest.trim().to_string();
            continue;
        }
        match trimmed.split_once('=') {
            Some((key, msg)) => {
                messages.insert(key.trim().to_string(), msg.trim().to_string());
            }
            None => {
                // A non-comment, non-header line without `=` is a malformed builtin asset.
                panic!("malformed builtin catalog line (no `=`): {trimmed:?}");
            }
        }
    }
    BuiltinCatalog { version, messages }
}

/// The built-in catalog for `lang`, if the build compiled it in (en/es today). An unknown/disabled
/// language returns `None` — the caller falls back to `en` (the never-blank chain).
pub fn builtin_for(lang: &str) -> Option<BuiltinCatalog> {
    match lang {
        "en" => Some(parse_builtin(EN_MF)),
        "es" => Some(parse_builtin(ES_MF)),
        _ => None,
    }
}

/// The built-in `en` catalog — the always-present fallback layer (every key that appears in
/// server-generated content is authored in en).
pub fn builtin_en() -> BuiltinCatalog {
    parse_builtin(EN_MF)
}

/// The version stamp echoed in verb responses for `lang` (the `en` builtin's version for an unknown
/// language, since that is what would be rendered).
pub fn builtin_version(lang: &str) -> String {
    builtin_for(lang).unwrap_or_else(builtin_en).version
}
