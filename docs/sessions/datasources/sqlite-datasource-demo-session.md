# Session — SQLite datasource first-class + the Docker-free demo dataset

Date: 2026-07-05 · Scope: [`../../scope/datasources/sqlite-datasource-demo-scope.md`](../../scope/datasources/sqlite-datasource-demo-scope.md)
· Promoted: [`../../public/datasources/datasources.md`](../../public/datasources/datasources.md) (§ SQLite, first-class)

## What shipped

1. **Seeder, SQLite edition** — `docker/postgres/seed.py --sqlite <path>` writes the SAME demo
   building dataset (the `inventory`/`generators`/`tags` brains, reused verbatim) into one `.db`
   file via a new sink module `docker/postgres/sinks_sqlite.py` (stdlib `sqlite3`, batched
   executemany, drop+recreate = the TRUNCATE-equivalent). Lite defaults when `--sqlite` is given
   and sizing flags are omitted: `--months 1 --interval 15` (~956k readings, ~15s, few-MB file);
   explicit flags still win, so the full-year firehose stays the postgres path. Sizing flags moved
   to `default=None` sentinels so per-sink defaults don't clobber explicit values.
2. **One-command demo** — `make seed-demo-sqlite` → `docker/postgres/seed-demo-sqlite.sh`:
   generates `.lazybones/data/demo/buildings.db` (the node data dir, per scope OQ1) and registers
   `demo-buildings {kind:"sqlite", dsn:<abs path>}` via login + the normal `datasource.add`
   (thecrew's seed-demo.sh pattern). `FED_ENDPOINTS` default grew `127.0.0.1:0` — the convention
   endpoint for file sources (no network endpoint) — so a fresh `make dev` node's install grant
   covers the demo's queries. Opt-in, not auto-seeded by `make dev` (scope OQ2 recommendation).
3. **First-class kind in the UI** — `AddDatasourceForm` kind is now a `<select>` over a `KINDS`
   data array (`postgres`/`timescale`/`sqlite` — the kinds `source/mod.rs::connect` accepts,
   hardcode-as-data per scope OQ3) with per-kind DSN + endpoint placeholders; picking `sqlite`
   prefills the `127.0.0.1:0` endpoint (editable) and shows the "path resolves on the node, not
   your browser" note.
4. **Honest missing-path probe (goal 4)** — `source/sqlite.rs::connect` now refuses a non-file
   path with "sqlite database file not found — the DSN path resolves on the node running the
   federation sidecar, not the client". Without this, SQLite silently CREATES an empty db that
   probes green (the remote-user-registers-a-laptop-path trap). The path (= the DSN) is never
   echoed in the error.

## Tests (all green this session)

- **`rust/crates/host/tests/federation_sqlite_test.rs`** (new; NO Docker — the sidecar builds with
  default features, sqlite isn't feature-gated, so it's a FAIL not a skip): real seeded `.db`
  fixture (rusqlite dev-dep, bundled) → probe green, `federation.schema` tables+columns,
  `federation.query` 3-row round-trip, path-DSN redaction in list+result, the missing-path error
  (asserts the message AND that no empty file was created), **capability-deny**, and
  **workspace-isolation** (mandatory categories). `cargo test -p lb-host --test
  federation_sqlite_test` → 1 passed.
- **UI** `DatasourcesAdmin.gateway.test.tsx` → 6/6 (real gateway): `addSource` helper updated to
  `selectOptions`; new sqlite case (kind select → endpoint prefills `127.0.0.1:0`, path DSN never
  rendered — redaction holds for a "less sensitive" path too).
- **Seeder**: `--sqlite` run twice with a pinned window → identical counts (idempotent); one
  meter's stored series compared element-wise against `generators.generate_scalar_series` for the
  same seed → exact match. Default lite run: 8 sites / 69 meters / 332 points / 956,160 readings.
- Full `pnpm test:gateway`: the datasources file is green; the 8 failing files (SystemView,
  sqlSource, App, inbox, workflow, panel, palette, ProofPanel, McpServiceView) are the branch's
  pre-existing failures — none touch datasources.

## Decisions

- **`127.0.0.1:0` convention endpoint** for file sources, pre-approved in the default dev grant —
  rejected alternative: exempting `kind:"sqlite"` from `enforce_endpoint`, which would put a
  kind-branch in a core mediation chokepoint (rule 10 leak) and weaken the net wall.
- **Missing-path = refuse in `sqlite.rs`** (the sanctioned one-file fix) rather than a host-side
  path check — the host never sees the DSN semantics per kind; the engine impl owns them.
- The path DSN gets **no redaction special case** (scope's secrets bullet), asserted in both tests.

## Follow-ups

- Data Studio 10x demo toggle (that scope's OQ2) can now point at `demo-buildings`.
- If a second engine family lands, revisit a `federation.kinds` discovery verb (scope OQ3).
