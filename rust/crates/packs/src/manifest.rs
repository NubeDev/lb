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

pub use crate::manifest_refs::EntityRef;
pub use crate::manifest_retention::{
    RetentionAlign, RetentionDeadband, RetentionFilter, RetentionPolicy, RetentionRange,
    RetentionTier,
};

/// One pack manifest as authored. `deny_unknown_fields` turns a typo'd key into a loud parse error
/// instead of a silently-ignored line — pack authors run `pack.validate` in CI, and a swallowed key
/// is exactly the bug that survives to production.
///
/// `Serialize` as well as `Deserialize`: `pack.get` hands the manifest back to a reader (the
/// embedder's Packs pages render it), so the shape must round-trip.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
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

    /// Series retention policies to set (`pack-retention-scope.md`). Each is applied via
    /// `series.retention.set` (LWW upsert keyed by `prefix`), so a pack shipping a high-rate
    /// producer (e.g. EMS's `modbus.*` polls) declares "keep raw for N, roll up, evict" instead of
    /// leaving every deployment to accumulate unbounded raw. Inline objects (the `channels:` model),
    /// NOT file refs — a policy is a small structured record. One receipt object per `prefix`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retention: Vec<RetentionPolicy>,

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
    /// Optional MAP hint (`map-widget-scope` Phase 2): this entity's rows can be drawn as pins on a
    /// geomap. A *projection*, like the binding — it names which columns carry the map's four facts
    /// (id/label/lat/lng) and, optionally, a child rollup for badge counts. Core stores no rows and
    /// generates no SQL from it; a downstream builder reads it off the receipt to fill a map cell,
    /// deriving the read verb from `backend` (store ⇒ `store.query`, datasource ⇒ `federation.query`).
    /// Absent ⇒ the entity is not mappable (today's shape). See [`GeoHint`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geo: Option<GeoHint>,
    /// Optional CHART recipes: what is worth plotting about ONE row of this entity. The exact sibling of
    /// `geo:` one step on — `geo:` says which columns place a row on a map, this says which reads chart
    /// it. A *projection* in the same sense: core stores nothing, derives nothing, and runs none of these
    /// queries. A downstream authoring surface reads them off the receipt to offer an author
    /// "☑ Energy · last 7 days" instead of asking for a dashboard id and a variable binding, and what it
    /// compiles is ordinary widget config — so a node that never applied the pack renders the result
    /// identically (rule 10). Absent ⇒ the entity offers no charts (today's shape). See [`ChartHint`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub charts: Vec<ChartHint>,
    /// Optional SOURCE REFS (`entity-source-refs-scope.md`): this entity's rows also exist, under the
    /// same ids, in a federation datasource — the high-resolution twin the store seed does not
    /// duplicate. The third sibling of `geo:`/`charts:` and the same projection discipline: an
    /// *address*, never behavior. Core carries it in the receipt, emits no SQL from it, and joins
    /// nothing across backends; a downstream surface resolves `source` by NAME against the viewer's
    /// registered datasources at read time and builds an ordinary `federation.query`. Declaring a ref
    /// grants nothing — the federation caps wall is unchanged. Absent ⇒ the entity has no declared
    /// twin (today's shape). See [`EntityRef`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<EntityRef>,
}

/// One `charts:` recipe on an [`Entity`]. A named, windowed read parameterised by the ROW — never a
/// rendered chart: the recipe says which rows and over what window, and the downstream surface decides
/// how to draw them.
///
/// The `var` + `query` pair is the load-bearing contract, and it is a **trust surface**: the row's id
/// must enter the query as a *variable reference* the consumer's own interpolator resolves
/// (`${site:sqlstring}`), never as a literal the pack author spliced or concatenated in. Core does not
/// and cannot enforce that — it never parses SQL — so the discipline is stated here and *checked by the
/// consumer*, which drops a recipe whose template does not reference its declared `var`. Keeping the
/// field pair explicit (rather than inferring the variable name) is what makes that check possible.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChartHint {
    /// Stable key within the entity — `energy`, `water`. What a downstream tick is addressed by, so it
    /// must not change once authored.
    pub key: String,
    /// Human label for the offered row.
    pub label: String,
    /// Default time window as a duration string (`7d`, `24h`). Interpreted downstream; absent ⇒ the
    /// consumer's own default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,
    /// The variable name the query parameterises the row id on. Absent ⇒ the consumer defaults it to the
    /// entity key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub var: Option<String>,
    /// Override datasource (a `datasource`-backed read). Absent ⇒ routed by the entity's `backend`.
    ///
    /// On a `backend: store` entity this is legal IFF it names one of the entity's declared
    /// [`EntityRef`]s (`entity-source-refs-scope.md` §4) — the payoff of `refs:`. The recipe's
    /// derive-path (`table`/`columns`/`kind`) then addresses the *datasource* table, and what compiles
    /// downstream is an ordinary `federation.query` cell parameterised by `${<var>:sqlstring}`. A
    /// dangling in-manifest reference (a `source` no ref declares) is the author's bug and gates at
    /// validate; whether the source exists in a given workspace is a late-bound *workspace* fact and
    /// never gates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The read, with the row id as a variable reference (see the type docs). Absent ⇒ the consumer
    /// derives it from `table` + `columns` below.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Table to derive the read from when `query` is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    /// Which columns of that table carry the series. Optional — each falls back downstream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<ChartColumns>,
    /// Optional `kind`-style discriminator for a table holding several series side by side (the
    /// denormalised-readings shape). Opaque to core.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// Which columns of a [`ChartHint`]'s table carry the series. Every key optional — the consumer's own
/// defaults apply, exactly as they do for [`GeoColumns`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChartColumns {
    /// The timestamp column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    /// The measured value column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// The column carrying the parent entity's id — what the recipe filters on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
}

/// The `geo:` map hint on an [`Entity`] (`map-widget-scope` Phase 2). Backend-agnostic: for a
/// `backend: store` entity the downstream builder derives `SELECT data FROM <table>` from the binding,
/// so `source`/`query` are only needed to OVERRIDE that (a `datasource` entity, or a hand-tuned read).
/// Every column falls back to the map form's own default downstream, so the minimal useful hint is an
/// empty `geo: {}` on a bound, coordinate-carrying entity.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeoHint {
    /// Override datasource for the rows (a `datasource`-backed entity). Absent for a store entity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Override read query (source A). Absent ⇒ the builder derives it from `backend` + `table`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Which columns carry the four map facts. Every key is optional (falls back downstream).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<GeoColumns>,
    /// Optional equip→site rollup backing per-pin badge counts. Absent ⇒ plain pins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollup: Option<GeoRollup>,
}

/// The column mapping inside a [`GeoHint`] — all optional, all defaulted downstream.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeoColumns {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lat: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lng: Option<String>,
}

/// The rollup source inside a [`GeoHint`] — a child query naming (equip, site) so open insights tally
/// onto their site pin. `source`/`query` optional the same way as the parent hint.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeoRollup {
    /// Override datasource for the rollup rows. Absent ⇒ same routing as the parent (store/datasource).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The rollup read query. For a store rollup the builder derives `SELECT data FROM <table>` when
    /// this is absent and a `table` is given; otherwise this is the read verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// The store table the rollup reads (store rollups) — lets the builder derive `SELECT data FROM t`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    /// Column naming the equip, and the column naming the site it belongs to. Optional/defaulted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<GeoRollupColumns>,
}

/// The (equip, site) column mapping inside a [`GeoRollup`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeoRollupColumns {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
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

/// The sidebar seed — the item refs a pack hides from, and the order it arranges, the workspace
/// rail. Each ref is opaque data in the shared nav grammar (a bare surface key like `channels`,
/// `ext:<id>`, `dashboard:<id>`, or a `group:<Label>` heading); the applier does not interpret them,
/// it hands each set to `nav.hidden.set` / `nav.order.set` verbatim. Rule 10: the arm branches on the
/// KIND, never on a named pack, and never on which surface a ref names.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Sidebar {
    /// The refs to hide (full set — LWW replaces, empty clears).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<String>,
    /// The rail ordering (full list — LWW replaces, empty clears). A PARTIAL order: a ref named here
    /// takes that position, anything unnamed keeps its natural order behind it, so a pack may arrange
    /// only the few entries it cares about without freezing the rest of the rail.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
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
    fn parses_the_geo_map_hint_and_rejects_a_typo() {
        // A store entity's map hint: no `source`/`query` (the builder derives `SELECT data FROM site`
        // from the binding); a column override and a store rollup naming its child table.
        let yaml = r#"
pack: ems
title: Energy Management
version: 1
entities:
  site:
    label: Site
    table: site
    pk: id
    backend: store
    geo:
      columns: { id: id, label: name, lat: lat, lng: lng }
      rollup:
        table: meter
        columns: { equip: id, site: site_id }
  meter:
    label: Meter
    parent: site
    table: meter
    pk: id
    backend: store
"#;
        let m = Manifest::parse(yaml).unwrap();
        let geo = m.entities["site"].geo.as_ref().expect("site declares geo");
        assert_eq!(geo.source, None); // store entity — routed by backend, no datasource
        assert_eq!(geo.query, None); // derived downstream from the binding
        let cols = geo.columns.as_ref().unwrap();
        assert_eq!(cols.lat.as_deref(), Some("lat"));
        let rollup = geo.rollup.as_ref().unwrap();
        assert_eq!(rollup.table.as_deref(), Some("meter"));
        assert_eq!(
            rollup.columns.as_ref().unwrap().site.as_deref(),
            Some("site_id")
        );

        // A minimal `geo: {}` on a bound entity is legal — every field defaults downstream.
        let bare = Manifest::parse(
            "pack: p\ntitle: P\nversion: 1\n\
             entities:\n  site: { label: Site, table: site, pk: id, backend: store, geo: {} }\n",
        )
        .unwrap();
        assert!(bare.entities["site"].geo.is_some());

        // A typo'd KEY inside `geo:` fails loudly (deny_unknown_fields on GeoHint).
        let err = Manifest::parse(
            "pack: p\ntitle: P\nversion: 1\n\
             entities:\n  site: { label: Site, geo: { colums: {} } }\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("colums"), "{err}");
    }

    #[test]
    fn parses_entity_chart_recipes_and_rejects_a_typo() {
        // `charts:` is `geo:`'s sibling — the same projection discipline, one step on. Core parses and
        // carries it; it never runs the query.
        let yaml = r#"
pack: p
title: P
version: 1
entities:
  site:
    label: Site
    table: site
    pk: id
    backend: store
    geo:
      columns: { id: id, label: name, lat: lat, lng: lng }
    charts:
      - key: energy
        label: Energy
        window: 7d
        var: site
        query: SELECT data.ts AS time, data.val AS v FROM reading WHERE data.site_id = ${site:sqlstring}
      - key: water
        label: Water
        table: reading
        kind: water
        columns: { time: ts, value: val, entity: site_id }
"#;
        let m = Manifest::parse(yaml).unwrap();
        let charts = &m.entities["site"].charts;
        assert_eq!(charts.len(), 2);

        // The explicit-template form: the row id enters as a VARIABLE REFERENCE, which is the whole
        // trust contract. Core does not check it (it never parses SQL) — it carries it verbatim so the
        // consumer can.
        assert_eq!(charts[0].key, "energy");
        assert_eq!(charts[0].window.as_deref(), Some("7d"));
        assert_eq!(charts[0].var.as_deref(), Some("site"));
        assert!(charts[0]
            .query
            .as_deref()
            .unwrap()
            .contains("${site:sqlstring}"));
        assert_eq!(charts[0].table, None);

        // The derive form: no `query`, so the consumer builds the read from table + columns + kind.
        assert_eq!(charts[1].table.as_deref(), Some("reading"));
        assert_eq!(charts[1].kind.as_deref(), Some("water"));
        let cols = charts[1].columns.as_ref().unwrap();
        assert_eq!(cols.time.as_deref(), Some("ts"));
        assert_eq!(cols.entity.as_deref(), Some("site_id"));
        assert_eq!(charts[1].query, None);

        // ABSENT is the today-shape and must stay legal + empty (not an error, not an Option dance).
        let bare = Manifest::parse(
            "pack: p\ntitle: P\nversion: 1\n\
             entities:\n  site: { label: Site, table: site, pk: id, backend: store }\n",
        )
        .unwrap();
        assert!(bare.entities["site"].charts.is_empty());

        // A typo'd KEY inside a recipe fails loudly (deny_unknown_fields on ChartHint) — the same
        // contract `geo:` keeps, so a mis-typed `windwo:` cannot silently mean "no window".
        let err = Manifest::parse(
            "pack: p\ntitle: P\nversion: 1\n\
             entities:\n  site: { label: Site, charts: [{ key: e, label: E, windwo: 7d }] }\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("windwo"), "{err}");

        // …and so does a MISSING required field: `key`/`label` are what a downstream tick addresses, so
        // a recipe without them is not a partially-useful recipe, it is unaddressable.
        let missing = Manifest::parse(
            "pack: p\ntitle: P\nversion: 1\n\
             entities:\n  site: { label: Site, charts: [{ label: E }] }\n",
        )
        .unwrap_err();
        assert!(missing.to_string().contains("key"), "{missing}");
    }

    #[test]
    fn a_manifest_round_trips_its_chart_recipes() {
        // The receipt is what a downstream surface actually reads, and it is SERIALIZED on the way out.
        // A field that parses but does not re-serialize would reach the consumer as "no charts" — the
        // silent failure this asserts against.
        let yaml = "pack: p\ntitle: P\nversion: 1\n\
                    entities:\n  site: { label: Site, charts: [{ key: energy, label: Energy, var: site, query: 'SELECT 1 WHERE id = ${site}' }] }\n";
        let m = Manifest::parse(yaml).unwrap();
        let out = serde_json::to_string(&m).unwrap();
        assert!(out.contains("\"charts\""), "{out}");
        let back: Manifest = serde_json::from_str(&out).unwrap();
        assert_eq!(back, m);
        assert_eq!(back.entities["site"].charts[0].label, "Energy");

        // An entity with no charts must NOT materialize an empty `charts: []` into the receipt — the
        // skip-if-empty wire contract every other additive field here keeps.
        let plain =
            Manifest::parse("pack: p\ntitle: P\nversion: 1\nentities:\n  site: { label: Site }\n")
                .unwrap();
        assert!(!serde_json::to_string(&plain).unwrap().contains("charts"));
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

    #[test]
    fn parses_a_retention_block_with_tiers() {
        let m = Manifest::parse(concat!(
            "pack: ems\ntitle: T\nversion: 1\n",
            "retention:\n",
            "  - prefix: \"modbus.\"\n",
            "    raw_for_ms: 3600000\n",
            "    max_samples: 5000\n",
            "    tiers:\n",
            "      - {width_ms: 60000, keep_for_ms: 604800000}\n",
        ))
        .unwrap();
        assert_eq!(m.retention.len(), 1);
        let p = &m.retention[0];
        assert_eq!(p.prefix, "modbus.");
        assert_eq!(p.raw_for_ms, 3_600_000);
        assert_eq!(p.max_samples, 5_000);
        assert_eq!(p.tiers.len(), 1);
        assert_eq!(p.tiers[0].width_ms, 60_000);
        assert_eq!(p.tiers[0].keep_for_ms, 604_800_000);
    }

    #[test]
    fn retention_defaults_and_empty_are_clean() {
        // No block ⇒ empty vec (the `#[serde(default)]` path).
        let none = Manifest::parse("pack: ems\ntitle: T\nversion: 1\n").unwrap();
        assert!(none.retention.is_empty());
        // A policy may omit optional fields (raw_for_ms/max_samples/tiers default to 0/0/[]).
        let bare =
            Manifest::parse("pack: ems\ntitle: T\nversion: 1\nretention:\n  - prefix: \"m.\"\n")
                .unwrap();
        assert_eq!(bare.retention[0].prefix, "m.");
        assert_eq!(bare.retention[0].raw_for_ms, 0);
        assert!(bare.retention[0].tiers.is_empty());
    }

    #[test]
    fn a_typod_key_inside_a_retention_policy_is_a_loud_error() {
        // `deny_unknown_fields` on `RetentionPolicy` — `raw_for_mss:` must not silently drop.
        let err = Manifest::parse(
            "pack: ems\ntitle: T\nversion: 1\nretention:\n  - prefix: \"m.\"\n    raw_for_mss: 1\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("raw_for_mss"), "{err}");
    }
}
