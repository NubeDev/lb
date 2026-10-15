//! The `typed_value:` block on an entity (`entity-typed-value-scope.md`) — the serde shape only.
//!
//! A settings-style entity keeps rows of different shapes in one table: a number on one row, a JSON
//! document on the next, a flag on a third. The block names the three columns that say what each
//! row's value IS — the row's `type` (`number` | `text` | `bool` | `json`, a downstream convention),
//! its `value`, and for a `json` row the JSON Schema that value must match.
//!
//! Like `table`/`geo`/`charts`/`refs` it is an **address, not behavior**. Core carries the block in
//! the receipt, reads no row, checks no value against its type or schema, and validates only shape
//! ([`crate::validate_typed`]). A downstream surface reads it off `pack.get` and picks the editor per
//! row. Checking data is the line `pack-entity-binding-scope.md` holds; it stays downstream.
//!
//! It lives beside [`crate::manifest`] rather than inside it for the same reason
//! [`crate::manifest_refs`] does: one responsibility per file.

use serde::{Deserialize, Serialize};

/// The columns of an entity's bound table that say what each row's value is.
///
/// `deny_unknown_fields`, like every other manifest struct: a typo'd `scheme:` must fail loudly
/// rather than silently mean "no schema column".
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypedValue {
    /// The column holding each row's type word. Core never reads it; the words are downstream's.
    pub r#type: String,
    /// The column holding each row's value — a number, text, a flag, or a JSON document as text.
    pub value: String,
    /// The column holding a `json` row's JSON Schema. Absent ⇒ the entity has no schema column, and a
    /// downstream surface edits `json` rows as raw JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
}

impl TypedValue {
    /// Every column the block names, in declaration order, as `(field, column)`.
    pub fn columns(&self) -> Vec<(&'static str, &str)> {
        let mut out = vec![
            ("type", self.r#type.as_str()),
            ("value", self.value.as_str()),
        ];
        if let Some(schema) = &self.schema {
            out.push(("schema", schema.as_str()));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use crate::manifest::Manifest;

    const WITH_TYPED: &str = r#"
pack: config
title: Configuration
version: 1
entities:
  site_setting:
    label: Site setting
    table: config_site_setting
    pk: id
    display: name
    typed_value:
      type: type
      value: value
      schema: schema
"#;

    #[test]
    fn parses_and_round_trips() {
        let m = Manifest::parse(WITH_TYPED).expect("parses");
        let t = m.entities["site_setting"]
            .typed_value
            .as_ref()
            .expect("typed_value");
        assert_eq!(
            (t.r#type.as_str(), t.value.as_str(), t.schema.as_deref()),
            ("type", "value", Some("schema"))
        );
        assert_eq!(
            t.columns(),
            [("type", "type"), ("value", "value"), ("schema", "schema")]
        );
        let again: Manifest = serde_json::from_value(serde_json::to_value(&m).unwrap()).unwrap();
        assert_eq!(again, m, "the receipt carries the block unchanged");
    }

    #[test]
    fn schema_is_optional() {
        let m = Manifest::parse(&WITH_TYPED.replace("      schema: schema\n", "")).expect("parses");
        let t = m.entities["site_setting"].typed_value.as_ref().unwrap();
        assert_eq!(t.schema, None);
        assert_eq!(t.columns(), [("type", "type"), ("value", "value")]);
    }

    #[test]
    fn absent_block_serializes_as_before() {
        let m = Manifest::parse(&WITH_TYPED.replace(
            "    typed_value:\n      type: type\n      value: value\n      schema: schema\n",
            "",
        ))
        .expect("parses");
        assert!(m.entities["site_setting"].typed_value.is_none());
        assert!(!serde_json::to_string(&m).unwrap().contains("typed_value"));
    }

    #[test]
    fn a_typo_is_refused() {
        let err =
            Manifest::parse(&WITH_TYPED.replace("schema: schema", "scheme: schema")).unwrap_err();
        assert!(err.to_string().contains("scheme"), "{err}");
    }
}
