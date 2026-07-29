//! The `refs:` block on an entity (`entity-source-refs-scope.md`) — the serde shape only.
//!
//! A store-backed entity often has a **twin in a federation datasource**: the same ids, carrying the
//! high-resolution history the store seed deliberately does not duplicate. Until this block existed
//! that identity was folklore (documented in a pack README, checked by nothing). A `ref` turns it into
//! contract by naming an *address*: "this entity's pk is also the key in datasource X, table Y,
//! column Z."
//!
//! Like `table`/`geo`/`charts` it is an **address, not behavior**. Core stores the block in the
//! receipt, generates no SQL from it, joins nothing across backends, and validates only shape
//! ([`crate::validate_refs`]). A downstream surface reads it off `pack.get` and builds an ordinary
//! `federation.query` parameterised by the entity variable that already exists — which is why a node
//! that never applied the pack renders the compiled result identically (rule 10).
//!
//! It lives beside [`crate::manifest`] rather than inside it for the same reason
//! [`crate::manifest_retention`] does: one responsibility per file.

use serde::{Deserialize, Serialize};

/// One declared twin of an entity's rows in a federation datasource.
///
/// `deny_unknown_fields`, like every other manifest struct: a typo'd `tabel:` must fail loudly rather
/// than silently mean "no table", because a ref that half-parses addresses the wrong rows.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EntityRef {
    /// The datasource **name** the workspace knows it by. Resolved LATE — at read time, against the
    /// viewer's registered datasources — exactly as every saved `federation.query` cell resolves its
    /// source. Whether it exists here is a *workspace* fact, never a pack fact, so it is never a
    /// validate gate (`entity-source-refs-scope.md` §Validation).
    pub source: String,
    /// The table in that source carrying this entity's twin rows.
    pub table: String,
    /// The column of `table` holding this entity's pk. Absent ⇒ the entity's own `pk` name (O-1: the
    /// 90% case is literal id parity, and the default keeps manifests honest-short).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fk: Option<String>,
    /// Optional human label for a downstream picker ("Interval readings (demo)").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl EntityRef {
    /// The column carrying the entity's pk — the declared `fk`, else the entity's own `pk` (O-1).
    /// Kept here rather than at each call site so the default is stated once and cannot drift.
    pub fn fk_or<'a>(&'a self, entity_pk: &'a str) -> &'a str {
        self.fk.as_deref().unwrap_or(entity_pk)
    }
}

#[cfg(test)]
mod tests {
    use crate::manifest::Manifest;

    const WITH_REFS: &str = r#"
pack: ems
title: Energy Management
version: 1
entities:
  site:
    label: Site
    table: ems_site
    pk: id
    backend: store
    refs:
      - source: demo-buildings
        table: site
        fk: id
        label: Interval data (demo)
    charts:
      - key: demand-hires
        label: Interval demand
        source: demo-buildings
        table: point_reading
        columns: { time: ts, value: val, entity: site_id }
        kind: demand
        window: 7d
"#;

    #[test]
    fn parses_the_refs_block() {
        let m = Manifest::parse(WITH_REFS).unwrap();
        let refs = &m.entities["site"].refs;
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].source, "demo-buildings");
        assert_eq!(refs[0].table, "site");
        assert_eq!(refs[0].fk.as_deref(), Some("id"));
        assert_eq!(refs[0].label.as_deref(), Some("Interval data (demo)"));
    }

    #[test]
    fn fk_defaults_to_the_entity_pk() {
        // O-1, decided: an omitted `fk` means literal id parity — the entity's own pk column name.
        let m = Manifest::parse(
            "pack: p\ntitle: P\nversion: 1\nentities:\n  site:\n    label: Site\n    \
             table: ems_site\n    pk: id\n    backend: store\n    \
             refs:\n      - { source: d, table: site }\n",
        )
        .unwrap();
        let ent = &m.entities["site"];
        assert_eq!(ent.refs[0].fk, None);
        assert_eq!(ent.refs[0].fk_or(ent.pk.as_deref().unwrap()), "id");
    }

    #[test]
    fn an_absent_refs_block_is_the_todays_shape() {
        // The optionality-is-the-safety-property rule: every existing pack parses unchanged, and an
        // entity with no refs must NOT materialize `refs: []` into the receipt.
        let m = Manifest::parse(
            "pack: p\ntitle: P\nversion: 1\nentities:\n  site: { label: Site, table: t, pk: id }\n",
        )
        .unwrap();
        assert!(m.entities["site"].refs.is_empty());
        assert!(!serde_json::to_string(&m).unwrap().contains("refs"));
    }

    #[test]
    fn a_typod_key_inside_a_ref_is_a_loud_error() {
        let err = Manifest::parse(
            "pack: p\ntitle: P\nversion: 1\nentities:\n  site:\n    label: Site\n    \
             refs:\n      - { source: d, tabel: site }\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("tabel"), "{err}");
    }

    #[test]
    fn a_missing_required_ref_field_is_a_loud_error() {
        // `source`/`table` ARE the address — a ref without them is not a partial address, it is none.
        let err = Manifest::parse(
            "pack: p\ntitle: P\nversion: 1\nentities:\n  site:\n    label: Site\n    \
             refs:\n      - { source: d }\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("table"), "{err}");
    }

    #[test]
    fn refs_round_trip_through_the_receipt_serialization() {
        // The receipt carries the manifest SERIALIZED; a field that parses but does not re-serialize
        // reaches the consumer as "no refs" — the silent failure this asserts against.
        let m = Manifest::parse(WITH_REFS).unwrap();
        let out = serde_json::to_string(&m).unwrap();
        assert!(out.contains("\"refs\""), "{out}");
        let back: Manifest = serde_json::from_str(&out).unwrap();
        assert_eq!(back, m);
        assert_eq!(back.entities["site"].refs[0].source, "demo-buildings");
    }
}
