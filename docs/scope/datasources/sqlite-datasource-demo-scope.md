# Datasources scope — SQLite datasource, first-class + the Docker-free demo dataset

Status: **SHIPPED** (2026-07-05) — promoted to `public/datasources/datasources.md` (§ SQLite,
first-class); session log: `../../sessions/datasources/sqlite-datasource-demo-session.md`.
Companion to [`../frontend/data-studio-10x-scope.md`](../frontend/data-studio-10x-scope.md)
(it answers that scope's OQ2 — the demo-data toggle — the lite way).

The federation sidecar **already speaks SQLite**: `source/sqlite.rs` is a shipped, real engine
behind the one `Source` trait (`kind: "sqlite"`, the documented test fallback when Docker is
unavailable). But nothing surfaces it: the Add-datasource form's kind is a free-text field (you'd
have to know to type `sqlite`), the DSN semantics for it (a **file path**, not a URL) are
documented nowhere, and the demo dataset exists only as `docker/postgres/seed.py` → a running
TimescaleDB container. The ask: make SQLite a **first-class datasource kind** in the UI, and emit
the **same demo building dataset into a SQLite file** — so "try Data Studio with real-looking
data" needs one command and zero Docker.

## Goals

1. **First-class kind in the Datasources UI.** `AddDatasourceForm` grows a kind **select**
   (`postgres`, `timescale`, `sqlite` — the kinds `source/mod.rs::connect` actually accepts,
   listed as data, not branched on) with a per-kind DSN placeholder: URL for postgres/timescale,
   **absolute file path** for sqlite (e.g. `/var/lib/lb/demo/buildings.db`). Probe, schema
   discovery (`federation.schema`), `federation.query`, and the Data Studio picker then work
   unchanged — the sidecar path is already engine-agnostic above the trait.
2. **The demo dataset, SQLite edition.** The postgres seeder already splits its brains into
   `inventory.py` (sites/meters/points), `generators.py` (per-meter seeded RNG physics), and
   `tags.py` — reuse them verbatim: a `--sqlite <path>` mode on `seed.py` (recommended over a
   second script — one dataset definition, two sinks) writes the SAME schema + rows to a `.db`
   file via Python's stdlib `sqlite3`. Default sizing is the lite profile: `--months 1
   --interval 15` (≈200k rows, seconds to generate, a few MB) — the full-year firehose stays the
   postgres/Timescale path.
3. **One-command demo:** `make seed-demo-sqlite` → generates the file under the node's data dir
   and registers it (`datasource.add {kind:"sqlite", dsn:<path>}`) in the target workspace via the
   normal admin verb. This is what the Data Studio 10x demo toggle points at: real records, real
   engine, no container.
4. **Document the DSN-is-a-path caveat** honestly: the path is resolved on the **node running the
   federation sidecar**, not the browser — a remote gateway user pointing at their laptop path
   gets a clean probe error naming this.

## Non-goals

- **Not a second platform datastore** (rule 2 intact): SQLite here is an EXTERNAL federated
  source behind the sidecar's `Source` trait — exactly like postgres. Platform state stays
  SurrealDB only; nothing core links `sqlite`.
- **No new sidecar engine work.** `sqlite.rs` ships already; if a gap surfaces (e.g. table
  discovery quirks) it's a fix in that one file, not new surface.
- **No write path.** Federation stays read-first; the seeder writes the file offline, the sidecar
  only reads it.
- **No bundling the `.db` in the repo/image.** It's generated (seconds), not vendored — a binary
  blob in git rots and bloats.

## How it fits the core

- **Tenancy / isolation:** unchanged — a datasource record is workspace-scoped
  (`datasource.add/list` already enforce it); the same file path registered in two workspaces is
  two independent grants. Isolation test extends the existing datasources suite.
- **Capabilities:** no new caps. `datasource.*` admin CRUD and `mcp:federation.query:call` gate
  exactly as today; the deny path is the shipped one. `make seed-demo-sqlite` authenticates like
  any admin caller.
- **Symmetric nodes:** kind list is config/data in the UI; the sidecar match on `kind` is the one
  sanctioned per-engine seam (one impl file per kind, per the federation scope).
- **Secrets:** the DSN (path) rides the same host secret mediation as a postgres DSN — never
  logged, never in results. A path is less sensitive but gets no special case.
- **No mocks (rule 9):** this scope is the *anti-mock* — demo data as real rows in a real engine
  queried through the real sidecar. The `Source` trait remains the one sanctioned external seam.
- **One responsibility per file:** kind select stays in `AddDatasourceForm` (it owns the form);
  DSN placeholder map is data in the same file; `seed.py` gains a sink module
  (`sinks_sqlite.py`) rather than inline branching if the diff gets large.
- **SDK/WIT / MCP surface:** none / consumed only. **Skill doc:** extend the existing federation
  workflow docs with the sqlite DSN form + seed command; no new SKILL.md (no new verb).

## Example flow

1. `make seed-demo-sqlite WS=acme` — generates `data/demo/buildings.db` (1 month, 15-min, ~70
   meters) and registers datasource `demo-buildings` (kind `sqlite`) in `acme`.
2. In Datasources, the row probes green; Tables discovery lists `sites/meters/points/readings`.
3. In Data Studio, the source picker shows `demo-buildings`; a builder tab's SQL prefills; Run
   returns real rows; the 10x demo toggle uses this source when the user's own query is empty.
4. A member without `mcp:federation.query:call` sees the datasource name but every query denies —
   the shipped deny path, now also covered for kind `sqlite`.

## Testing plan

- **Mandatory:** capability-deny (query denied without the federation cap against a sqlite
  source) + workspace-isolation (a sqlite datasource registered in ws-A invisible/denied in
  ws-B) — extensions of the existing real-gateway datasources tests, no new harness.
- Sidecar: a seeded-fixture `.db` (generated in the test, small) → `probe`, `federation.schema`
  (tables + columns), `federation.query` round-trip; a missing-path probe returns the honest
  node-local-path error (goal 4).
- Seeder: `seed.py --sqlite` twice → identical row counts (idempotent TRUNCATE-equivalent);
  spot-check one meter's series matches the postgres generator output for the same seed.
- UI gateway: Add-datasource with the kind select (`sqlite` + path) lands the record; the
  existing add-form test updates from free-text to select.

## Risks & hard problems

- **Path semantics across deployments** (the real trap): in Docker the sidecar's filesystem is
  the container's — the seed target dir must be a mounted volume the compose file already
  declares, or the demo silently 404s. The Makefile target owns putting the file where the
  sidecar can see it.
- **SQLite + DataFusion pushdown:** the table-provider path may push less down than postgres
  (fine at demo sizes; state the lite profile is the point — don't let anyone seed a year into
  SQLite and file a perf bug).
- **Concurrent readers:** N builder tabs querying one file — SQLite handles concurrent reads,
  but open the pool read-only to keep the contract explicit.

## Open questions — all resolved as recommended (2026-07-05)

1. **Resolved:** node data dir — `make seed-demo-sqlite` writes `.lazybones/data/demo/buildings.db`.
2. **Resolved:** opt-in (`make seed-demo-sqlite`); `make dev` does not auto-seed. The default
   `FED_ENDPOINTS` did grow `127.0.0.1:0` (the file-source convention endpoint) so a fresh dev
   node's install grant covers the demo's queries without a re-install.
3. **Resolved:** hardcode-as-data — the `KINDS` array in `AddDatasourceForm` (per-kind DSN +
   endpoint placeholders); a `federation.kinds` discovery verb waits for a second engine family.

## Related

- Parent: [`datasources-scope.md`](datasources-scope.md) (the federation extension) ·
  demo consumer: [`../frontend/data-studio-10x-scope.md`](../frontend/data-studio-10x-scope.md) (OQ2)
- Code: `rust/extensions/federation/src/source/{mod,sqlite}.rs` (shipped),
  `ui/src/features/datasources/AddDatasourceForm.tsx`, `docker/postgres/{seed,inventory,generators,tags}.py`
- Public: [`../../public/datasources/datasources.md`](../../public/datasources/datasources.md)
