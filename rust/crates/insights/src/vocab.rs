//! `tag_vocab` — the workspace's own declared vocabulary for a tag key, and the raise-time
//! validation of `category` against it (`docs/scope/insights/case-plane-scope.md` §"Vocabulary").
//!
//! **This is the rule-10 seam.** A closed `category` set is what makes a roster groupable and a
//! case routable, but the SET ITSELF is workspace data, not lb's knowledge: a pack seeds a
//! `tag_vocab:{key}` row declaring its values, and lb enforces whatever it finds there. lb ships
//! **no default value list** — grep this crate for a category value and you will find none. An
//! unseeded workspace therefore validates **nothing**: every value is accepted, exactly as before
//! this file existed, so adding it breaks no existing workspace and encodes no product taxonomy.
//!
//! The key name `category` IS named here. That is the tag GRAMMAR (which dimension carries the
//! closed set), not a value — the same altitude as `Insight::severity` naming a field. The values
//! that grammar admits are the workspace's business.
//!
//! One responsibility: the vocabulary record + the two questions the raise path asks it.

use lb_store::{read, Store};
use serde::{Deserialize, Serialize};

use crate::error::InsightsError;

/// The store table workspace vocabularies live in. One row per tag key; the record id IS the key.
pub const TABLE: &str = "tag_vocab";

/// The tag key whose vocabulary the raise path enforces. The grammar, not a value (module doc).
pub const CATEGORY_KEY: &str = "category";

/// One workspace-declared vocabulary. Seeded by packs, editable in settings; every field is
/// `#[serde(default)]` so a row that declares only `values` (the common case) decodes, and so does
/// a row written before a field existed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagVocab {
    /// The tag key this vocabulary is for (mirrors the record id).
    #[serde(default)]
    pub key: String,
    /// The declared, closed set of legal values. **Empty ⇒ open** — an undeclared vocabulary
    /// validates nothing rather than rejecting everything (the difference between an additive
    /// feature and a workspace that can no longer raise).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<String>,
    /// The subset of [`TagVocab::values`] that **gate** other findings — a value in this list marks
    /// a finding whose subject matter undermines confidence in findings derived from the same data.
    /// The caveat stamp reads this (see [`TagVocab::gating_values`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gates: Vec<String>,
    /// An explicit override naming the ONE gating value, when a workspace wants `gates` to mean
    /// something else. Absent ⇒ [`TagVocab::gates`] is the gating set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_quality_category: Option<String>,
}

impl TagVocab {
    /// The values that gate other findings — the explicit `data_quality_category` if the workspace
    /// declared one, else every value in `gates`. **Empty ⇒ the caveat stamp is a no-op**, which is
    /// the state of every workspace that has not opted in.
    ///
    /// A `Vec` rather than a single value because a workspace may legitimately declare more than
    /// one gating category, and collapsing them to the first would silently ignore the rest.
    pub fn gating_values(&self) -> Vec<String> {
        match &self.data_quality_category {
            Some(v) => vec![v.clone()],
            None => self.gates.clone(),
        }
    }
}

/// Read the workspace's declared vocabulary for `key`. `Ok(None)` ⇒ the workspace declared none
/// (open — see the module doc). A row that fails to decode is treated as absent rather than as an
/// error: a malformed vocabulary must not make the workspace unable to raise.
pub async fn read_vocab(
    store: &Store,
    ws: &str,
    key: &str,
) -> Result<Option<TagVocab>, InsightsError> {
    let Some(value) = read(store, ws, TABLE, key).await? else {
        return Ok(None);
    };
    Ok(serde_json::from_value::<TagVocab>(value).ok())
}

/// Validate a `category` value against the workspace's declared set.
///
/// - No `tag_vocab:category` row, or a row declaring no `values` ⇒ **Ok** (unseeded is open).
/// - A value inside the declared set ⇒ **Ok**.
/// - A value outside it ⇒ [`InsightsError::BadInput`] **naming the declared set**, so the producer
///   author sees what they may have meant instead of a bare rejection.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Vocabulary"
pub async fn validate_category(store: &Store, ws: &str, value: &str) -> Result<(), InsightsError> {
    check_value(read_vocab(store, ws, CATEGORY_KEY).await?.as_ref(), value)
}

/// The PURE half of [`validate_category`], over an already-read vocabulary. `raise` reads the row
/// once (it needs [`TagVocab::gating_values`] from the same row for the caveat stamp) and calls
/// this, so the hot path costs one store read rather than two.
pub fn check_value(vocab: Option<&TagVocab>, value: &str) -> Result<(), InsightsError> {
    let Some(vocab) = vocab else {
        return Ok(()); // unseeded ⇒ open
    };
    if vocab.values.is_empty() || vocab.values.iter().any(|v| v == value) {
        return Ok(());
    }
    Err(InsightsError::BadInput(format!(
        "category {value:?} is not in this workspace's declared {CATEGORY_KEY} vocabulary [{}]",
        vocab.values.join(", ")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gating set falls back to `gates`, and an explicit override wins — the smallest honest
    /// seam, tested so the fallback cannot be dropped as "unused".
    #[test]
    fn gating_values_prefers_the_explicit_declaration() {
        let v = TagVocab {
            key: CATEGORY_KEY.into(),
            values: vec!["a".into(), "b".into(), "c".into()],
            gates: vec!["b".into(), "c".into()],
            data_quality_category: None,
        };
        assert_eq!(v.gating_values(), vec!["b".to_string(), "c".to_string()]);

        let v = TagVocab {
            data_quality_category: Some("c".into()),
            ..v
        };
        assert_eq!(v.gating_values(), vec!["c".to_string()]);

        assert!(
            TagVocab::default().gating_values().is_empty(),
            "a workspace that declared nothing gates nothing"
        );
    }
}
