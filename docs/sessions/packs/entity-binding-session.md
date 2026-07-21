# Session — the entity→table binding + federation write/delete for the data plane

**Date:** 2026-07-21. **Scope:** `docs/scope/packs/pack-entity-binding-scope.md` (this repo, the
keystone). Origin: the downstream consumer `NubeIO/rubix-ai → entity-data-plane-scope.md` (operator row
CRUD + generated entity pages + entity var). This session lands the CORE half; rubix-ai consumes it via
a local `[patch]` and will bump the `node-v*` pin on release (handled externally).

## O-1 (the gating unknown) — SETTLED: `federation.write` reaches the in-process sqlite source

`federation.write` is documented for a registered EXTERNAL source and gates `net:*` on the endpoint. A
pack's datasource is the in-process sqlite materializer (node-local file DSN). **Proven live that it
reaches it — no new write path is needed.**

`pack.apply` registers a materialized sqlite datasource with `kind="sqlite"`, `endpoint="127.0.0.1:0"`
(the node-local convention, `apply.rs:apply_datasource`), DSN = the canonical `.db` path. `datasource_add`
self-grants `net:tls:127.0.0.1:0:connect`, so `enforce_endpoint` passes; the write then rides the SAME
`resolve → mediate_dsn → call_sidecar → run_write → SqliteSource::write_rows` path `federation.query`
already uses on these sources. Test:
`crates/host/tests/pack_test.rs::o1_federation_write_reaches_the_pack_materialized_sqlite_source`
(`#[ignore]` — needs the real federation sidecar; run with `cargo build -p federation` +
`FEDERATION_BIN=…`). It writes a new site, reads it back, UPSERTs idempotently, and edits a seeded row.

## Seed ownership (the re-apply decision) — implemented + proven

Per scope §"seed-ownership decision": seeded ROWS are starting data, applied ONCE, never re-clobbered;
schema DDL stays migratable. `materialize()` previously deleted the file on every apply.

- `apply_datasource` now receives the first-apply signal (`run_rules`) and branches:
  first apply → `sqlite::materialize()` (fresh, schema + seed); re-apply → `sqlite::resolve_existing()`
  (the existing file, rows untouched, re-registered). `resolve_existing` returns `Ok(None)` when the file
  was purged, and the caller self-heals by rebuilding fresh (no operator data to protect).
- **Decision:** re-apply does NOT re-run the authored `CREATE TABLE` DDL — it would fail on the existing
  db, and schema evolution is the additive `federation.migrate` path. Idempotence ("apply twice, same end
  state") holds for the data half exactly as for every other object kind.
- Proven by O-1 step 4: after re-apply, the operator's added row AND their edit to a seeded row survive.

## Phase B — the binding (manifest + receipt + validate)

- `crates/packs/src/manifest.rs`: `Entity` gains optional `table`/`pk`/`parent_fk`/`display`,
  `deny_unknown_fields`, `skip_serializing_if` (an unbound entity serializes byte-for-byte as before).
- **Receipt carry** is free: the receipt stores the whole `manifest`, so `pack.get` returns the binding
  with no new verb/envelope. Test: `pack_test.rs::the_entity_table_binding_rides_the_receipt_to_pack_get`
  (bound fields present + an unbound field ABSENT, not null-spammed).
- **Validate** (`crates/packs/src/binding.rs`, pure, 8 unit tests; wired into `validate.rs`):
  `parent_fk` with no `parent` is an ERROR that gates (manifest-only inconsistency, like a dangling
  parent). Everything schema-referential (unknown table/column) is a WARNING — a pack's schema can be
  opaque (postgres) and the real oracle is the apply (the dialect-lint precedent). A small dialect-blind
  `CREATE TABLE` scanner reads the pack's own `schema.sql`; unparseable → opaque → warns nothing.
  Test: `pack_test.rs::a_malformed_binding_warns_but_only_parent_fk_without_parent_gates`.

## O-2 — `federation.delete` (new verb, same caps/shape as `federation.write`)

Row DELETE had no verb (the query validator rejects DELETE). Added a first-class, bounded, STRUCTURED
`federation.delete {source, table, key, rows}` — the caller names key columns + key-aligned value rows,
never SQL. Mirrors `federation.write` at every layer:
- Engine: `crates/federation/src/delete.rs` (`run_delete`, ROW_CAP 1000, identifier validation) +
  `Source::delete_rows` in `source/mod.rs` implemented in `source/sqlite.rs` (parameterized
  `DELETE … WHERE k=?n`, one transaction) and `source/postgres.rs`. Evicts the result cache like writes.
- Sidecar dispatch: `crates/federation/src/main.rs` (`"federation.delete" => …`).
- Host: `crates/host/src/federation/delete.rs` (`federation_delete` + `delete_descriptor`), registered in
  `federation/mod.rs`, dispatched in `federation/tool.rs`, in `tools/descriptor.rs` + `system/catalog.rs`.
- Caps: `mcp:federation.delete:call`, re-checked host-side, workspace-resolved.
- Test: `crates/host/tests/federation_sqlite_test.rs::federation_delete_removes_a_row_by_key` —
  delete-by-key + read-back-gone, capability-deny (opaque), and a bad-identifier refusal.

## Test commands

```
cd rust
cargo build -p federation
cargo test -p lb-packs                                   # binding + validate unit tests
cargo test -p federation delete                          # engine unit tests
FEDERATION_BIN=$(pwd)/target/debug/federation \
  cargo test -p lb-host --test federation_sqlite_test federation_delete -- --nocapture
FEDERATION_BIN=$(pwd)/target/debug/federation \
  cargo test -p lb-host --test pack_test -- --ignored --nocapture   # O-1 + demo oracle
cargo test -p lb-host --test pack_test                   # receipt-carry + validate (non-ignored)
cargo fmt --check
```

## Deferred

- **Phase E / `U-entity-foreach`** (feed the operator-managed entity set into a rule as a `for_each`
  input): a rules-engine change, named in the scope, NOT built this session.

Release: cut a `node-v*` tag; rubix-ai bumps `lb-node` (handled externally). Local `[patch]` from
rubix-ai points at this checkout during dev.
