//! The `pack.yaml` serde shape — the authored contract of a domain pack, and nothing else. No
//! validation beyond what serde enforces, no I/O; the linter lives in [`crate::validate`] and the
//! object plan in [`crate::plan`].
//!
//! ⚠ The `entities` block is UNSTABLE until a runtime consumer exists (pack-core-scope's own
//! warning: it stays a *vocabulary*, never an ORM); nothing here promises a compatibility contract
//! yet.
//!
//! Ported verbatim from the proving prototype (`NubeIO/rubix-ai` `crates/pack-apply/src/manifest.rs`)
//! — the format shipped and was live-verified before it moved into core.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One pack manifest as authored. `deny_unknown_fields` turns a typo'd key into a loud parse error
/// instead of a silently-ignored line — pack authors run `pack.validate` in CI, and a swallowed key
/// is exactly the bug that survives to production.
///
/// `Serialize` as well as `Deserialize`: `pack.get` hands the manifest back to a reader (the
/// embedder's Packs pages render it), so the shape must round-trip.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// The pack id — the stable name (`bas`, `ems`, …). The receipt is keyed by it.
    pub pack: String,
    /// Human title for a reader ("Building Automation").
    pub title: String,
    /// Monotonic version. Bumped on any change; the receipt records what it applied, and the
    /// refusal matrix (higher = not-yet-built, lower = always refused) keys off this.
    pub version: u32,

    /// The noun vocabulary everything binds by (site → equip → point …). Documentation + the
    /// future picker source; NOT applied to any seam.
    #[serde(default)]
    pub entities: BTreeMap<String, Entity>,

    /// The insight-key grammar (dedup-key patterns + severities). Documentation; not applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insights: Option<Insights>,

    /// The datasource to register, with its optional schema/seed SQL. For a pack whose entities live
    /// in the STORE (`pack-store-datasource-scope.md`), this block is TIME-SERIES only (or absent) —
    /// entity rows are seeded via the top-level `seed_rows`, not here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub datasource: Option<Datasource>,

    /// STRUCTURED seed rows for STORE-backed entities (`pack-store-datasource-scope.md` O-1): a
    /// bundle-relative path to a JSON object `{ "<table>": [ {<row fields>}, … ], … }`. This is a
    /// TOP-LEVEL concern, independent of any `datasource` block — the rows seed the ONE application
    /// store (SurrealDB) directly, so a store-only pack needs no datasource at all. Each row is
    /// UPSERT'd at `<table>:<pk>` on FIRST apply, run-once (seed-ownership); the pk column comes from
    /// the entity binding that names the table. The store takes structured values, never SQL (mirrors
    /// `federation.write`'s no-SQL contract) — a store pack ships `seed_rows`, a sqlite pack ships
    /// `datasource.seed`, and a pack may ship BOTH (store entities + a federation readings table).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_rows: Option<String>,

    /// The name of a PRIOR sqlite datasource whose entity rows should be MIGRATED into the store on
    /// first apply (`pack-store-datasource-scope.md` §Migration). A pack that USED to bind its
    /// `site`/`meter`/`point` to a sqlite datasource, and now binds them `backend: store`, names that
    /// old datasource here — so a workspace that already CRUD'd the sqlite rows carries the OPERATOR's
    /// live rows into the store (read the live rows, never the seed), not just the pack's fresh seed.
    /// Absent ⇒ no migration (a pack that was always store-backed). The migration runs BEFORE the seed
    /// and only into an empty store table (never clobbering); a failed migration leaves the sqlite
    /// file in place (no half-move).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migrate_from: Option<String>,

    /// Rhai rules to save, and run once on first apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<String>,

    /// Dashboards to save. Cells are pre-bound to the vocabulary by the author.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dashboards: Vec<String>,

    /// Channels to create.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<Channel>,

    /// The agent's domain context — a path to a markdown file, applied as durable
    /// workspace-shared agent memory. The sharpest clobber edge: never overwritten silently.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<Agent>,

    /// The workspace sidebar seed — a subtractive hidden-set applied via `nav.hidden.set`
    /// (full-set LWW). Declutter, never authz: hiding a surface never blocks its route (the
    /// gateway re-checks every verb on click). One object per workspace, keyed by the pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidebar: Option<Sidebar>,

    /// Required extension ids — CHECKED against the installed set, never installed (installing is
    /// the admin's act; the pack only declares needs). An absent requirement warns, never blocks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<String>,
}

/// One entity in the vocabulary tree. Deliberately a *vocabulary*, not an ORM: `parent` is the only
/// relation, `kinds`/`units` are flat hints. The moment it grows behavior it is a NEW scope.
///
/// The optional **entity→table binding** (`table`/`pk`/`parent_fk`/`display`,
/// `pack-entity-binding-scope.md`) is a *projection*, not behavior: it names which table an entity's
/// rows live in, so a downstream surface can address them through the `federation.*` verbs. An entity
/// with no `table` is exactly the shape-only vocabulary — the promise is unbroken. The binding stores
/// no rows, generates no SQL in core, and enforces nothing about the data (that line is where a NEW
/// scope begins — `unique`/`required`/`computed`/a second FK are all "not a fifth field").
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entity {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub units: BTreeMap<String, String>,
    /// The table this entity's rows live in (binding). Absent ⇒ shape-only vocabulary (today's shape).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    /// The primary-key column of `table` — the UPSERT key a downstream row editor writes on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pk: Option<String>,
    /// The column of `table` that references the PARENT entity's row (drill-down through the forest).
    /// Requires the entity to declare `parent`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_fk: Option<String>,
    /// The human-label column of `table` — the roster/picker label a downstream surface shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    /// Which backend this entity's rows live in (`pack-store-datasource-scope.md`): `store` (a
    /// SurrealDB record via the `store.*` verbs — the one application store, Data-browser-visible,
    /// graph-linkable, caps-scopable) or `datasource` (the federation/sqlite path, for time-series or
    /// a registered external source). Absent ⇒ the pack's `datasource.engine` decides (O-2:
    /// `store` engine ⇒ store; `sqlite`/`postgres` ⇒ datasource), so existing packs keep working.
    /// The receipt carries this so a downstream surface routes without guessing (rule 10: route on
    /// the binding, never on a pack/entity name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<Backend>,
}

/// The storage backend an entity's rows live in. `deny_unknown_fields` on `Entity` already rejects a
/// typo'd KEY; this enum rejects a typo'd VALUE (`bakend: stor` fails to parse loudly rather than
/// silently defaulting), the same loud-error posture the whole manifest holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    /// A SurrealDB record via the `store.*` verbs (the one application store).
    Store,
    /// The federation datasource (sqlite/postgres/…) via the `federation.*` verbs.
    Datasource,
}

/// The insight-key grammar block.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Insights {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<InsightKey>,
}

/// One dedup-key pattern (e.g. `fdd:{issue}:{equip}`) + the severities it raises.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InsightKey {
    pub pattern: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub severity: Vec<String>,
}

/// The datasource declaration (+ schema/seed executed into the source before registration).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Datasource {
    pub name: String,
    /// The federation kind (`sqlite`, `postgres`, …). Schema/seed SQL applies only where the host
    /// can materialize the source; other kinds register only.
    pub engine: String,
    /// Optional DDL file (a bundle-relative path). Dialect-intersection rules apply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// Optional seed SQL file (a bundle-relative path). Used by the `sqlite` materialize path only.
    /// (Store-backed entity rows are seeded via the top-level `seed_rows`, not here.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<String>,
}

/// A channel to register.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Channel {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// The agent context declaration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Agent {
    /// Bundle-relative path to the markdown context file.
    pub context: String,
}

/// The sidebar seed — the set of item refs a pack hides from the workspace rail. Each ref is
/// opaque data in the shared nav grammar (a bare surface key like `channels`, `ext:<id>`, or
/// `dashboard:<id>`); the applier does not interpret them, it hands the set to `nav.hidden.set`
/// verbatim. Rule 10: the arm branches on the KIND, never on a named pack, and never on which
/// surface a ref names.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Sidebar {
    /// The refs to hide (full set — LWW replaces, empty clears).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<String>,
}

impl Manifest {
    /// Parse a manifest from YAML text. Errors carry `serde_yaml`'s line/column.
    pub fn parse(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_minimal_manifest() {
        let m = Manifest::parse("pack: bas\ntitle: Building Automation\nversion: 1\n").unwrap();
        assert_eq!(m.pack, "bas");
        assert_eq!(m.title, "Building Automation");
        assert_eq!(m.version, 1);
        assert!(m.rules.is_empty());
    }

    #[test]
    fn an_unknown_top_level_key_is_a_loud_error() {
        // `deny_unknown_fields`: a typo'd key must not be silently ignored.
        let err = Manifest::parse("pack: bas\ntitle: T\nversion: 1\nrulez: []\n").unwrap_err();
        assert!(
            err.to_string().contains("rulez"),
            "the error names the unknown key: {err}"
        );
    }

    #[test]
    fn a_missing_required_field_errors_with_a_line() {
        let err = Manifest::parse("title: T\nversion: 1\n").unwrap_err();
        assert!(
            err.to_string().contains("pack"),
            "the error names the missing field: {err}"
        );
    }

    #[test]
    fn parses_the_full_entity_and_datasource_blocks() {
        let yaml = r#"
pack: bas
title: Building Automation
version: 1
entities:
  site:
    label: Site
  equip:
    label: Equipment
    parent: site
    kinds: [ahu, chiller]
    units:
      zone-temp: degC
datasource:
  name: demo-buildings
  engine: sqlite
  schema: schema.sql
  seed: seed.sql
channels:
  - name: critical-faults
    description: "Critical FDD raises land here."
agent:
  context: agent-context.md
"#;
        let m = Manifest::parse(yaml).unwrap();
        assert_eq!(m.entities["equip"].parent.as_deref(), Some("site"));
        assert_eq!(m.entities["equip"].kinds, vec!["ahu", "chiller"]);
        assert_eq!(m.datasource.as_ref().unwrap().name, "demo-buildings");
        assert_eq!(m.channels[0].name, "critical-faults");
        assert_eq!(m.agent.as_ref().unwrap().context, "agent-context.md");
    }

    #[test]
    fn parses_the_entity_backend_and_rejects_a_bad_value() {
        let m = Manifest::parse(
            "pack: p\ntitle: P\nversion: 1\n\
             entities:\n  site: { label: Site, table: site, pk: id, backend: store }\n",
        )
        .unwrap();
        assert_eq!(m.entities["site"].backend, Some(Backend::Store));

        // A typo'd VALUE fails to parse loudly (not a silent default).
        let err = Manifest::parse(
            "pack: p\ntitle: P\nversion: 1\n\
             entities:\n  site: { label: Site, backend: stor }\n",
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("backend") || err.to_string().contains("stor"),
            "{err}"
        );

        // Absent backend is the today's shape (engine decides downstream).
        let m2 =
            Manifest::parse("pack: p\ntitle: P\nversion: 1\nentities:\n  site: { label: Site }\n")
                .unwrap();
        assert_eq!(m2.entities["site"].backend, None);
    }

    #[test]
    fn parses_a_top_level_seed_rows_with_a_store_datasource() {
        let m = Manifest::parse(
            "pack: p\ntitle: P\nversion: 1\n\
             seed_rows: seed.json\n\
             datasource:\n  name: d\n  engine: store\n",
        )
        .unwrap();
        assert_eq!(m.seed_rows.as_deref(), Some("seed.json"));
        assert_eq!(m.datasource.unwrap().engine, "store");
    }

    #[test]
    fn parses_seed_rows_with_no_datasource_at_all() {
        // A store-only pack needs no datasource block — `seed_rows` is a top-level concern.
        let m = Manifest::parse(
            "pack: p\ntitle: P\nversion: 1\n\
             entities:\n  site: { label: Site, table: site, pk: id, backend: store }\n\
             seed_rows: seed.json\n",
        )
        .unwrap();
        assert_eq!(m.seed_rows.as_deref(), Some("seed.json"));
        assert!(m.datasource.is_none());
    }

    #[test]
    fn parses_a_sidebar_hidden_block() {
        let m = Manifest::parse(
            "pack: bas\ntitle: T\nversion: 1\n\
             sidebar:\n  hidden:\n    - channels\n    - datasources\n",
        )
        .unwrap();
        assert_eq!(
            m.sidebar.as_ref().unwrap().hidden,
            vec!["channels", "datasources"]
        );
    }

    #[test]
    fn a_typod_key_inside_sidebar_is_a_loud_error() {
        // `deny_unknown_fields` on `Sidebar` too — `hiddn:` must not silently apply nothing.
        let err =
            Manifest::parse("pack: bas\ntitle: T\nversion: 1\nsidebar:\n  hiddn: [channels]\n")
                .unwrap_err();
        assert!(err.to_string().contains("hiddn"), "{err}");
    }
}
