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
//! One responsibility: the vocabulary record + the questions the raise path asks it, and the
//! validated write an admin surface needs to author one.
//!
//! **Why the write is a verb and not a raw store row.** The vocabulary is an ENGINE-OWNED record:
//! `raise` refuses a category outside `values`, the caveat stamp reads `gates`, and the case plane
//! keys its SLA matrix on the same set. A UI writing it through the generic `store.write` door can
//! produce a row this file then has to trust — and the generic entity grid demonstrably does exactly
//! that for JSON-shaped columns, writing `["a"]` as the STRING `"[\"a\"]"`, which decodes to
//! nothing and silently disarms the validation. [`validate_vocab`] + [`write_vocab`] are the door
//! that cannot be got round.

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

/// The most values one vocabulary may declare. A bound, not a policy: a closed set with a thousand
/// members is not a closed set, and every roster that groups by this key would become unreadable.
pub const MAX_VOCAB_VALUES: usize = 64;

/// Validate a vocabulary BEFORE any write, so a rejected set leaves the declared one exactly as it
/// was.
///
/// Deliberately thin, and it judges SHAPE, never meaning: a key, non-blank values, no duplicates,
/// and a `gates` list drawn from the values it gates. Nothing here decides what a category IS —
/// that is the workspace's business and the whole reason this record exists (rule 10).
///
/// The `gates ⊆ values` rule is the one with teeth: a gating value that is not a declarable value
/// can never be raised, so the caveat mechanism it arms would be silently dead. That is exactly the
/// failure this plane cannot afford, because a caveat that never fires looks identical to a
/// workspace with nothing to caveat.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Vocabulary"
pub fn validate_vocab(vocab: &TagVocab) -> Result<(), InsightsError> {
    if vocab.key.trim().is_empty() {
        return Err(InsightsError::BadInput("a vocabulary needs a key".into()));
    }
    if vocab.key.contains(':') {
        // The key IS the store record id (`tag_vocab:{key}`); a `:` would split the record key.
        return Err(InsightsError::BadInput(
            "a vocabulary key may not contain `:` — it is a single record-id segment".into(),
        ));
    }
    if vocab.values.len() > MAX_VOCAB_VALUES {
        return Err(InsightsError::BadInput(format!(
            "a vocabulary may declare at most {MAX_VOCAB_VALUES} values, got {}",
            vocab.values.len()
        )));
    }
    for value in &vocab.values {
        if value.trim().is_empty() {
            return Err(InsightsError::BadInput(
                "a vocabulary value may not be blank".into(),
            ));
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    for value in &vocab.values {
        if !seen.insert(value.as_str()) {
            return Err(InsightsError::BadInput(format!(
                "duplicate vocabulary value {value:?}"
            )));
        }
    }
    for gate in &vocab.gates {
        if !vocab.values.iter().any(|v| v == gate) {
            return Err(InsightsError::BadInput(format!(
                "gating value {gate:?} is not one of the declared values — a gate nothing can be \
                 raised as arms a caveat that never fires"
            )));
        }
    }
    if let Some(explicit) = &vocab.data_quality_category {
        if !vocab.values.iter().any(|v| v == explicit) {
            return Err(InsightsError::BadInput(format!(
                "data_quality_category {explicit:?} is not one of the declared values"
            )));
        }
    }
    Ok(())
}

/// Write `vocab` as this workspace's declaration for `vocab.key`, after [`validate_vocab`].
///
/// An EMPTY `values` list is legal and meaningful: it re-opens the key (lb's own rule — an
/// undeclared vocabulary validates nothing). Deleting the row and declaring nothing are the same
/// statement, so there is no separate delete verb to disagree with this one.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Vocabulary"
pub async fn write_vocab(store: &Store, ws: &str, vocab: &TagVocab) -> Result<(), InsightsError> {
    validate_vocab(vocab)?;
    let value = serde_json::to_value(vocab)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    lb_store::write(store, ws, TABLE, &vocab.key, &value).await?;
    Ok(())
}

/// Every vocabulary this workspace declares, ordered by key — the admin surface's roster read.
pub async fn list_vocab(store: &Store, ws: &str) -> Result<Vec<TagVocab>, InsightsError> {
    let rows = lb_store::scan_all(store, ws, TABLE).await?;
    let mut out: Vec<TagVocab> = rows
        .into_iter()
        // Unwrap the `{ data, rev }` write envelope the store scan returns — the flat fields live
        // under `data`. Every field on `TagVocab` is `#[serde(default)]`, so a row read WITHOUT
        // this unwrap decodes silently into an all-default vocabulary (key `""`, no values) rather
        // than failing: the roster would come back the right length and say nothing.
        .filter_map(|row| unwrap_vocab(row.data))
        .collect();
    out.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(out)
}

/// Unwrap the store's `{ data, rev }` envelope and decode. A row that will not decode is skipped
/// rather than failing the roster — one malformed declaration must not hide the others.
fn unwrap_vocab(row: serde_json::Value) -> Option<TagVocab> {
    let inner = match row {
        serde_json::Value::Object(mut obj) => {
            obj.remove("data").unwrap_or(serde_json::Value::Object(obj))
        }
        other => other,
    };
    serde_json::from_value(inner).ok()
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
