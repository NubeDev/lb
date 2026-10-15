//! One entity of the `entities:` block (`pack-core-scope`, `pack-entity-binding-scope.md`) — the
//! serde shape only. Split out of [`crate::manifest`] (which re-exports it) when the entity grew its
//! fifth projection block, the `manifest_refs.rs` precedent: one responsibility per file.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::manifest::{Backend, ChartHint, GeoHint};
use crate::manifest_refs::EntityRef;
use crate::manifest_typed::TypedValue;

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
    /// Optional TYPED VALUE (`entity-typed-value-scope.md`): the columns of `table` that say what each
    /// row's value is — its type word, its value, and a `json` row's JSON Schema — so one settings
    /// table serves rows of every shape. The fifth projection beside `geo:`/`charts:`/`refs:`, and the
    /// same discipline: core carries it, reads no row, and checks no value against it. Absent ⇒
    /// today's shape. See [`TypedValue`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_value: Option<TypedValue>,
}
