//! The **catalog layer** — MF1 message rendering on top of the shipped `format::*` (i18n-catalogs
//! scope, prefs Phase 2). Pure: no I/O, no store, no auth. Four one-responsibility pieces:
//!   - [`message`] — the hand-written MF1 subset parser + renderer.
//!   - [`plural`] — hand-encoded en/es CLDR plural categories (the icu4x swap point).
//!   - [`interpolate`] — routes typed placeholders to `format::*` (never re-derives formatting).
//!   - [`builtin`] — the compiled-in en/es `.mf` catalogs + the language→en→key fallback chain.
//!
//! [`render`] is the headline: `render(key, args, override, resolved) -> Rendered` selects the
//! message (override-shadows-builtin, then builtin `lang`, then builtin `en`, then the key literal),
//! parses it, and renders it against the recipient's resolved prefs. It **never panics, never
//! blanks** — an unparseable/absent message falls to the key; a bad placeholder renders `[<arg>]`.

mod builtin;
mod interpolate;
mod message;
mod plural;

use std::collections::BTreeMap;

use serde_json::Value;

use crate::prefs::ResolvedPrefs;

pub use builtin::{
    builtin_en, builtin_for, builtin_version, parse_builtin, BuiltinCatalog, EN_MF, ES_MF,
};
pub use message::{parse, ParseError};
pub use plural::PluralCategory;

/// The result of a catalog render — the text plus which locale actually supplied the message and the
/// catalog version stamp (the `message.render` verb's response shape).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub text: String,
    pub locale_used: String,
    pub catalog_version: String,
}

/// Render `key` against `args` and the recipient's `resolved` prefs, consulting `override_` (a
/// per-workspace sparse override map for the resolved language) first.
///
/// Selection order (the pinned fallback chain, never blank):
///   1. `override_[key]` — the workspace override for the resolved language,
///   2. the built-in catalog for the resolved language,
///   3. the built-in `en` catalog,
///   4. the key literal itself.
/// A message that fails to parse (should never happen for a builtin — lint guards that) also falls
/// through to the next layer, so a bad tenant override can never break rendering.
pub fn render(
    key: &str,
    args: &Value,
    override_: &BTreeMap<String, String>,
    resolved: &ResolvedPrefs,
) -> Rendered {
    let lang = resolved.language.as_str();
    let version = builtin_version(lang);

    // 1. workspace override for the resolved language.
    if let Some(src) = override_.get(key) {
        if let Ok(msg) = parse(src) {
            return Rendered {
                text: message::render(&msg, args, resolved),
                locale_used: lang.to_string(),
                catalog_version: version,
            };
        }
    }
    // 2. built-in for the resolved language.
    if let Some(cat) = builtin_for(lang) {
        if let Some(src) = cat.messages.get(key) {
            if let Ok(msg) = parse(src) {
                return Rendered {
                    text: message::render(&msg, args, resolved),
                    locale_used: lang.to_string(),
                    catalog_version: version,
                };
            }
        }
    }
    // 3. built-in `en` fallback.
    let en = builtin_en();
    if let Some(src) = en.messages.get(key) {
        if let Ok(msg) = parse(src) {
            return Rendered {
                text: message::render(&msg, args, resolved),
                locale_used: "en".to_string(),
                catalog_version: en.version,
            };
        }
    }
    // 4. the key literal — never blank, never panic.
    Rendered {
        text: key.to_string(),
        locale_used: lang.to_string(),
        catalog_version: version,
    }
}

/// The MERGED (override-over-builtin) catalog map for `locale` — what `prefs.catalog` returns for a
/// rich client. An unknown locale merges over the `en` builtin (never empty, the no-block rule). The
/// returned map is flat dotted keys → MF1 message source.
pub fn merged_catalog(
    locale: &str,
    override_: &BTreeMap<String, String>,
) -> (BTreeMap<String, String>, String) {
    let base = builtin_for(locale).unwrap_or_else(builtin_en);
    let mut merged = base.messages;
    for (k, v) in override_ {
        merged.insert(k.clone(), v.clone());
    }
    (merged, base.version)
}

/// Lint a catalog map: every message MUST parse as the pinned MF1 subset, else it is an authoring
/// error (i18n-catalogs scope: "an authored message using an out-of-subset construct fails a
/// build-time catalog-lint test, not at render"). Returns the first `(key, error)` offender.
pub fn lint(messages: &BTreeMap<String, String>) -> Result<(), (String, ParseError)> {
    for (key, src) in messages {
        parse(src).map_err(|e| (key.clone(), e))?;
    }
    Ok(())
}
