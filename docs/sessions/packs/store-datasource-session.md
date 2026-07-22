# Session — the `store` datasource kind for packs (the keystone)

Date: 2026-07-22. Scope: rubix-ai `docs/scope/packs/pack-store-datasource-scope.md` (the ask),
consumed here in **lb** as the upstream keystone. Owning-repo split (WORKFLOW-LB §2): the pack
engine's seed path + the binding's `backend` carry live in lb; rubix-ai consumes them via the local
`[patch]` (no tag/bump this session).

## What shipped (lb)

A pack can now declare `datasource: { engine: store }` (and/or entity bindings with
`backend: store`), and its entity rows are seeded as **SurrealDB records in the one application
store** via the generic `store.write` verb — NOT a node-local sqlite file. This is the exact shape
the EMS extension already writes (`NubeIO/ems → site_reach/store.rs`); packs now do the same.

Files:
- `crates/packs/src/manifest.rs` — `Entity` gains `backend: Option<Backend>` (`store` | `datasource`,
  `deny_unknown_fields` so a typo'd value is a loud parse error). `Datasource` gains
  `seed_rows: Option<String>` (a bundle-relative JSON path).
- `crates/packs/src/bundle.rs` — resolves `seed_rows` JSON `{table: [rows]}` into
  `Pack.seed_rows: BTreeMap<String, Vec<Value>>`; `parse_seed_rows` is loud on the two authoring
  mistakes (non-object root, non-object row).
- `crates/packs/src/plan.rs` — the datasource checksum + `content_checksum` fold `seed_rows`, so an
  edited store seed is drift (same "changed content at same version is refused" contract).
- `crates/packs/src/validate.rs` — an ERROR gates a `seed_rows` table with no bound entity carrying a
  `pk` (manifest-only, readable); WARNINGs for schema/seed SQL on a `store` engine.
- `crates/host/src/pack/store_seed.rs` — NEW. Mirrors `sqlite.rs`'s shape. `seed_rows(node, principal,
  ws, seed, pk_for)` UPSERTs each row at `{table}:{pk}` through `crate::store_write_run` under the
  CALLER's principal — so `store:<table>:write` is re-checked per row (no privileged path).
- `crates/host/src/pack/apply.rs::apply_datasource` — a `store` branch beside the sqlite one. The
  store seed is INDEPENDENT of the engine and runs only on first apply (seed-ownership); a pure
  `store` engine has no external source to `datasource_add`, so it returns `applied` after the seed.
  On first apply it runs the MIGRATION (below) before the seed.
- `crates/host/src/pack/store_seed.rs::migrate_sqlite_entities` — the MIGRATION path
  (`pack-store-datasource-scope.md` §Migration). A pack that names a prior sqlite `migrate_from`
  datasource carries the operator's LIVE sqlite entity rows (read live, not the seed) into EMPTY store
  tables — never clobbering, and the sqlite file is left in place (no half-move). The seed then SKIPS
  any table the migration filled (seed-ownership is per-table), so operator rows win over the seed.
- `crates/host/src/pack/mod.rs` — registers `store_seed`.

### Manifest shape (settled)
- `seed_rows` is a TOP-LEVEL manifest field (a store-only pack needs no datasource block), plus
  `migrate_from: Option<String>` naming a prior sqlite datasource to sweep. `Datasource.seed_rows` was
  NOT used — store seeding is decoupled from the datasource.
- `crates/host/tests/pack_store_test.rs` — NEW, 6 real-node tests (mem:// store, NO federation
  sidecar, so they ride the DEFAULT `cargo test`): seeds SurrealDB records, shows in `store.tables`,
  a `meter.site_id → site` edge is followable, seed-ownership holds across a re-apply/upgrade, a
  missing `store:<table>:write` is a denied partial, an unbound seed table gates at validate.

## Decisions (O-1 / O-2 outcomes)

- **O-1 — structured seed, not SQL.** A `store` pack ships `seed_rows` (a JSON `{table: [rows]}`
  file), never `seed.sql`. The store takes structured values (mirrors `federation.write`'s no-SQL
  contract); a translation shim from `seed.sql` was rejected as reintroducing SQL where the store
  wants none. The pk column comes from the entity binding (`entity.table == table` → its `pk`), so
  the record id IS the pk (`table:id`).
- **O-2 — engine-driven default, per-entity override.** `datasource.engine: store` ⇒ the datasource
  seeds the store. The per-entity `backend` (carried in the receipt) is what the CONSUMER routes on
  (Stage 2). Absent `backend` ⇒ engine decides. The two are separable: a pack may seed entity tables
  into the store AND keep a sqlite/federation datasource for time-series (`point_reading`) in the
  same manifest — the store seed runs whenever `seed_rows` is present, regardless of engine, and the
  sqlite materialize runs whenever the engine is sqlite with schema/seed SQL. This is the "entities
  in the store, time-series in federation" line, expressible in one manifest.

## The one load-bearing store fact (for Stage 2)

Store records live as `{ data: <fields>, rev }` (`crates/store/src/record.rs`) — the host JSON is
wrapped under `data` to dodge the serde_json↔SurrealDB enum-tag mismatch. So a store read for the
grid must `SELECT data FROM <table>` and UNWRAP the `data` envelope (the raw record `id` is a
SurrealDB Thing that won't deserialize to `serde_json::Value`; use `meta::id(id)` if the id is
needed as a column). The rubix-ai `entityBackend` store read must do this unwrap to keep the grid's
`{columns, rows}` shape identical.

## Tests

`cargo test -p lb-packs` (43 passed) + `cargo test -p lb-host --test pack_store_test` (7 passed —
incl. the sqlite→store migration no-loss/no-clobber test) + `cargo test -p lb-host --test pack_test`
(15 passed, 3 ignored — the sqlite oracle/O-1 need the federation binary). `cargo fmt --check` clean.

**Proven live (rubix-ai E2E, 2026-07-22):** a fresh isolated node applied `packs/ems` (store-backed);
`SELECT data FROM site|meter|…` returned the seeded records, they showed in `GET /store/tables`, a
`meter.site_id → site` edge resolved, `store.write`/`.delete` CRUD worked, and an operator edit
survived a re-apply. See `rubix-ai …/docs/sessions/packs/pack-store-datasource-session.md`.
