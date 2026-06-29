# Session — federation sidecar crash-loop on the Datasources page (discovery)

**Date:** 2026-06-29
**Area:** datasources / federation native extension (Tier-2 sidecar)
**Symptom (UI):** Opening a datasource (e.g. `timescale`) showed
`extension error: supervisor: restart budget exhausted after 5 restarts`, and the
Tables panel never populated.

## What was actually wrong

Three distinct defects, all on the discovery (`federation.schema`) path the
Datasources page drives on first load. The sidecar was **not** crashing at
spawn/init — `init` and `datasource.test` worked. The page's first action (list
tables) failed, and the way it failed exhausted the supervisor restart budget.

1. **Build shipped a sqlite-only binary.** `build.sh` ran
   `cargo build -p federation` with no `--features postgres`, so every
   Postgres/Timescale call returned `postgres source not built in`. The feature is
   off by default in `Cargo.toml` (it pulls native-tls → openssl).
   **Fix:** `build.sh` now builds `--features postgres` by default, with a
   `FEDERATION_NO_POSTGRES=1` escape hatch for TLS-less environments.

2. **Catalog discovery used `TableReference::bare` for dotted names.** The
   discovery path registered `pg_catalog.pg_tables` / `pg_catalog.pg_class` as
   table providers via `TableReference::bare("pg_catalog.pg_tables")`. `bare`
   keeps the literal dotted string as a *single* table name, so the provider
   resolved to an **empty schema** (zero columns). The catalog SELECT then failed
   with `No field named …` (`schemaname`, `c.relname`). `SELECT * FROM pg_tables`
   returned `columns: []`.
   **Fix:** use `TableReference::parse_str` for dotted remote/catalog names
   (splits into schema + table) in `source/postgres.rs` (the probe) and in
   `query.rs::catalog_rows` (the discovery binding). Also simplified the
   Postgres list-tables query to names-only from `pg_tables` (dropped the fragile
   `pg_class.reltuples` row-estimate join — a nice-to-have, not worth failing the
   whole listing).

3. **`federation.schema` was denied at the outer MCP gate.** `dispatch_at_depth`
   gates host-native verbs under `mcp:<tool>:call` keyed on the literal tool name,
   so `federation.schema` demanded `mcp:federation.schema:call` — a cap **no role
   grants** (the service layer in `schema.rs` deliberately re-checks the *query*
   cap, "discovery is the same read privilege"). Result: the browse panel was
   denied (opaque) even for a caller holding `mcp:federation.query:call`.
   **Fix:** alias `federation.schema`'s outer gate to `federation.query` in
   `tool_call.rs`.

Bonus: the extension manifest declared `federation.mirror` (a host-side verb the
child never serves) and omitted `federation.schema` (the verb it does serve).
Corrected `extension.toml` to match `main.rs`.

## Verification (real Postgres, no mocks)

- Direct stdio probe of the built sidecar against the dev TimescaleDB
  (`127.0.0.1:5433`, `lb/lb_secret/lb`):
  - `datasource.test` → `{ok:true}`
  - `federation.schema` (list) → `meter, point, point_reading, site`
  - `federation.schema` (describe `site`) → columns `id`, `name`
  - `federation.query` `SELECT * FROM site LIMIT 3` → real rows
- `cargo test -p lb-host --test federation_test` (real spawned `postgres:16-alpine`
  via the native supervisor + real sidecar): **ok, 1 passed** — now includes a
  discovery regression (list tables + describe columns through the real
  `call_tool` → `call_federation_tool` dispatch).

## Files touched

- `rust/extensions/federation/build.sh` — build `--features postgres` by default.
- `rust/extensions/federation/src/source/postgres.rs` — `parse_str` for the probe.
- `rust/extensions/federation/src/query.rs` — `parse_str` for catalog bindings;
  simplified Postgres list-tables query; dropped unused `Arc` import.
- `rust/extensions/federation/extension.toml` — declare `federation.schema`, drop
  `federation.mirror`.
- `rust/crates/host/src/tool_call.rs` — gate `federation.schema` under the query cap.
- `rust/crates/host/tests/federation_test.rs` — discovery regression assertions.

## To pick up the fix in a running dev box

Rebuild the sidecar binary and restart the node (the supervisor spawns the binary
from the workspace target dir, not from `.lazybones`):

```
make federation              # or: cargo build -p federation --features postgres
make dev                     # restart the node; re-installs + supervises the sidecar
```
