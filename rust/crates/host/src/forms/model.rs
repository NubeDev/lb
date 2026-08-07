//! The form record (forms scope, "Data"). A form is a workspace-namespaced `form:{id}` asset holding
//! a typed definition (`def` — the `options.form` shape: schema/ui/submit/mode/recordSource/
//! optionsSources/success) plus the owner and a soft-delete tombstone. It mirrors the [`Dashboard`]
//! record shape (a simple owner/workspace asset), but a form needs no visibility tier or cell
//! validation — the definition is opaque to the host beyond serde, so it is just persisted.
//!
//! [`Dashboard`]: crate::dashboard::Dashboard

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Deserialize a defaulted field tolerating an explicit JSON `null` (AI callers emit `"deleted": null`
/// where a human omits the key). `#[serde(default)]` alone only covers the ABSENT key; this covers
/// both — the same discipline the dashboard record uses.
fn null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + serde::Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// The table forms live in. Record id is `form:{id}` (the id is a stable slug, unique per workspace).
pub const TABLE: &str = "form";

/// Our form-document version, pinned on [`Form::schema_version`] at save. `1` = the first shape
/// (`def` = the `options.form` object). Bumped only when the stored document shape changes — the same
/// discipline as [`crate::dashboard::model::SCHEMA_VERSION`].
pub const SCHEMA_VERSION: u32 = 1;

/// A form record. The persisted definition + ownership metadata (forms scope, "Data").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Form {
    /// Stable slug, unique per workspace (the record id `form:{id}`).
    pub id: String,
    pub title: String,
    /// The form definition — the `options.form` shape (schema/ui/submit/mode/recordSource/
    /// optionsSources/success). Opaque to the host beyond serde; additive/defaulted so a titles-only
    /// save round-trips a pre-def record unchanged.
    #[serde(default)]
    pub def: Value,
    /// The principal who created it (the ownership anchor — only the owner may update or delete).
    pub owner: String,
    /// OUR form-document version — pinned at save (forms scope). Additive/defaulted; distinct from
    /// any version inside `def`.
    #[serde(default, rename = "schemaVersion")]
    pub schema_version: u32,
    pub updated_ts: u64,
    /// Tombstone (soft-delete, idempotent). A deleted form is hidden from `list`/`get`.
    #[serde(default, deserialize_with = "null_default")]
    pub deleted: bool,
}

/// The cheap roster row `list` returns — id/title/updated_ts, **no definition body** (the roster stays
/// cheap; forms scope, "Get / list").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormSummary {
    pub id: String,
    pub title: String,
    pub updated_ts: u64,
}

impl From<&Form> for FormSummary {
    fn from(f: &Form) -> Self {
        Self {
            id: f.id.clone(),
            title: f.title.clone(),
            updated_ts: f.updated_ts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A model-authored record with an explicit `null` `deleted` deserializes to the same default an
    /// absent key gets (the null-tolerance the dashboard record proved necessary for AI callers).
    #[test]
    fn form_tolerates_explicit_null_deleted() {
        let f: Form = serde_json::from_value(serde_json::json!({
            "id": "f1", "title": "Intake", "owner": "user:test", "updated_ts": 10,
            "deleted": null
        }))
        .expect("null deleted decodes to default");
        assert!(!f.deleted);
        assert!(f.def.is_null(), "absent def defaults to null");
    }

    /// The summary carries id/title/updated_ts and NOTHING of the definition body (the roster is cheap).
    #[test]
    fn summary_is_the_cheap_roster_row() {
        let f = Form {
            id: "f1".into(),
            title: "Intake".into(),
            def: serde_json::json!({ "schema": {} }),
            owner: "user:test".into(),
            schema_version: SCHEMA_VERSION,
            updated_ts: 42,
            deleted: false,
        };
        let s = FormSummary::from(&f);
        assert_eq!(s.id, "f1");
        assert_eq!(s.title, "Intake");
        assert_eq!(s.updated_ts, 42);
    }
}
