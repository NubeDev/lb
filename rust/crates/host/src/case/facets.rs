//! What a case echoes FROM its primary insight — severity, the three workspace facets, and whether
//! the finding is caveated (case-plane scope).
//!
//! One file because it is one decision stated once: a case's `severity`/`category`/`site`/`scope`
//! are **echoes**, not caller input. They are already properties of the detection, so a second
//! writer for them is a second thing that can disagree — the discipline `producer` and the tag echo
//! already hold.
//!
//! **Rule 10.** The three facet KEYS (`category`, `site`, `scope`) are lb's own dimension names,
//! documented in the case-plane scope's data model. Their VALUES are workspace vocabulary (a
//! `tag_vocab` row a pack seeds) and nothing here knows one: this reads whatever string the graph
//! holds and hands it on.
//!
//! `caveated` is read GENERICALLY out of the serialized record rather than off a typed field, so
//! this compiles and behaves correctly both before and after `lb-insights` grows its `caveats`
//! field — an absent field simply means "no caveats", which is the honest answer for a record
//! written before it landed.

use lb_insights::Insight;

/// The facet keys a case echoes. lb's own dimension names (see the module doc), not values.
const CATEGORY: &str = "category";
const SITE: &str = "site";
const SCOPE: &str = "scope";

/// What a case takes from its primary insight at open time.
#[derive(Debug, Clone, Default)]
pub(super) struct CaseFacets {
    pub severity: String,
    pub category: Option<String>,
    pub site: Option<String>,
    pub scope: Option<String>,
    pub caveated: bool,
}

/// Read the echo set off `insight`.
pub(super) fn facets_of(insight: &Insight) -> CaseFacets {
    CaseFacets {
        severity: severity_str(insight),
        category: insight.tags.get(CATEGORY).cloned(),
        site: insight.tags.get(SITE).cloned(),
        scope: insight.tags.get(SCOPE).cloned(),
        caveated: caveated(insight),
    }
}

/// The insight's severity as the lowercase string `lb-cases` ranks. Goes through serde rather than
/// a match so it can never drift from the wire form the enum actually serializes to.
pub(super) fn severity_str(insight: &Insight) -> String {
    serde_json::to_value(insight.severity)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// True when the insight carries open data-quality caveats. Read generically — see the module doc.
fn caveated(insight: &Insight) -> bool {
    serde_json::to_value(insight)
        .ok()
        .and_then(|v| v.get("caveats").cloned())
        .and_then(|v| v.as_array().map(|a| !a.is_empty()))
        .unwrap_or(false)
}
