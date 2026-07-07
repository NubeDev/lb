# Datasources (public)

Status: **SHIPPED** (2026-06-28; SQLite first-class + Docker-free demo 2026-07-05). Scope:
`../../scope/datasources/datasources-scope.md` +
`../../scope/datasources/sqlite-datasource-demo-scope.md`. Sessions:
`../../sessions/datasources/datasources-session.md` + `../../sessions/datasources/federation-session.md`
+ `../../sessions/datasources/sqlite-datasource-demo-session.md`.
Source/attribution: the embedded-DataFusion + SQL-validator pattern is reused from `rubix-cube`
(MIT/Apache-2.0) — crate-level comment in `extensions/federation`.

A native (Tier-2) **`federation` extension** that embeds DataFusion + connectors as a **library** to
query external SQL sources (Postgres/Timescale today; the DataFusion-federated set behind the same
`Source` trait), under `net:*` + a mediated secret, exposed as the read-first, **workspace-pinned**
`federation.query` MCP verb (plus `datasource.*` admin CRUD and a `federation.mirror` `lb-jobs` batch).
**SurrealDB stays the authoritative store** (rule 2 — it is NEVER a DataFusion source); external DBs
are federated sources reached only through the gated extension, never a second authority or sync peer.

## Architecture (one author surface, a correct split beneath)

To a rule author `source("series")` (platform) and `source("timescale")` (external) read alike (see
`../rules/rules.md`). Underneath, the federation extension is a **supervised, admin-approved,
`net:*`-gated process** that owns the sockets + the heavy engine — never the symmetric node binary
(core links no DB driver, rule 1). The engine + drivers live in one place behind one `Source` trait
(one impl per kind); the DSN lives only inside the pool, handed in per call by the host.

## MCP surface

| Verb | Cap | Does |
|---|---|---|
| `federation.query {source, sql}` | `mcp:federation.query:call` | read-first, workspace-pinned, **SELECT-only validated**, row-capped → `{columns, rows}`. The `{source}` resolves only to a `datasource:{ws}:{name}` in the **caller's** workspace (un-spoofable). |
| `datasource.add {name, kind, endpoint, secret_ref}` | admin cap | register a source; admin approves the `net:*` + `secret:*` at install. |
| `datasource.remove {name}` | admin cap | deregister. |
| `datasource.list {}` | `mcp:datasource.list:call` | registered sources — **no secrets in the output**. |
| `datasource.test {source}` | `mcp:datasource.test:call` | a real connectivity probe (green/red). |
| `federation.mirror {source, query, target_series, range}` | `mcp:federation.mirror:call` | a durable, resumable `lb-jobs` batch that reads the external range and `ingest.write`s it into the platform series plane → `{job_id}`. |

## The walls

- **`net:*` at connect** — the supervisor enforces `requested ∩ admin_approved` before opening a
  socket; a source whose endpoint the grant omits is refused **opaque, even with the binary installed**
  (`ext-loader::grant` does per-endpoint intersection so an admin-approved concrete endpoint satisfies
  a manifest's wildcard request). The cap is the new `net:tls/host/port:connect` grammar.
- **Secret mediation** — the DSN is `secret:federation/{source}` in `lb-secrets`, pulled by the host
  under the extension's own grant and handed to the pool; it is **never** in a record, a log, the page,
  a `datasource.list`, or a query result (a redaction assertion proves it).
- **Single-owner DSN (CRUD invariant)** — every DSN secret is **owned by the stable `ext:federation`
  principal**, never the (varying) admin who ran `datasource.add`. Write/read/delete all mediate as
  `ext:federation`, so the secrets **owner wall** (gate 3: overwrite/delete are owner-only) is a no-op
  between successive admins: any admin — a dev login, the boot seed, a future IdP user — may
  add/update/remove a source without collision. `add` writes via `lb_secrets::reclaim`
  (write-and-take-ownership; gates 1+2 still enforced), so a store poisoned by an earlier bootstrap
  owner **self-heals** on the next add/update; `remove` forgets the secret so a re-add starts clean.
  Coupling ownership to the caller was a real bug — every cross-admin update/remove collapsed to an
  opaque `denied` (see `debugging/datasources/test-denied-secret-owner-wall-across-admins.md`). A
  missing DSN now surfaces as a distinct `SecretUnavailable`, not a capability deny.
- **SELECT-only** — a write/DDL is rejected (read-first v1) both host-side and in the sidecar
  validator; a `federation.write` is a separate, later, Ask-gated verb.
- **Workspace wall** — `datasource:{ws}:{name}` is workspace-keyed; ws-B can neither name nor reach a
  ws-A source, and a mirror job's callback `ws` is host-set.

## SQLite, first-class + the Docker-free demo dataset

`kind:"sqlite"` is a first-class datasource kind (same `Source` trait, one impl file —
`source/sqlite.rs`; built into the DEFAULT sidecar, no postgres feature/TLS toolchain needed).
The **DSN is the database file path** (optionally `file:`-prefixed), resolved **on the node running
the federation sidecar — never the client**: a missing path is refused with an error saying exactly
that (SQLite would otherwise silently create an empty db that probes green). A sqlite source has no
network endpoint; it registers at the `127.0.0.1:0` convention endpoint, which `make dev`'s default
`FED_ENDPOINTS` pre-approves. The Add-datasource form's kind is a select
(`postgres`/`timescale`/`sqlite` — listed as data, never branched on in core) with per-kind DSN
placeholders; picking sqlite prefills the convention endpoint. Redaction has **no
path-is-less-sensitive special case** — the path DSN is mediated like any other.

**The demo, one command, zero Docker:** `make seed-demo-sqlite` generates the demo building dataset
(`docker/postgres/seed.py --sqlite` → the SAME `inventory`/`generators`/`tags` brains, the
`sinks_sqlite.py` sink; lite profile `--months 1 --interval 15`, ≈1M readings in seconds) into
`.lazybones/data/demo/buildings.db` and registers it as `demo-buildings` via the normal
`datasource.add`. Probe green, `federation.schema` discovery, Data Studio picker — all the shipped
paths, real rows in a real engine (rule 9's anti-mock). The full-year firehose stays the
postgres/Timescale seeder.

## Saved queries on the Datasources page (shipped 2026-07-06)

The Datasources detail page can save and reload ad-hoc SQL, riding the platform's existing `query.*`
verbs — **no new verb, cap, or table** (`querydef.*` was considered and dropped as a duplicate store):

- `ui/src/features/datasources/useDatasourceQueries.ts` — per-source hook. `query.list` returns the
  workspace roster; the hook filters **client-side** to `target === "datasource:<name>"` (a pure
  projection, no second call) and exposes load/save/remove. Saves are `lang:"raw"` (this surface
  authors raw SQL against the external engine; PRQL belongs to the platform-target workbench) with the
  datasource target baked in.
- `SaveQueryDialog.tsx` — id/name/description form; author supplies the slug, the current editor SQL +
  target are captured; submit closes, errors surface verbatim.
- `SavedQueriesDialog.tsx` — lists the filtered roster; click loads the SQL into the editor (the roster
  row omits `text`, so the full record is resolved via `query.get`); per-row delete.
- `DatasourceDetail.tsx` wires both dialogs into the SQL editor header, beside Run.

A saved query is a workspace-scoped `query:{ws}:{id}` record; its `datasource:<name>` target resolves
only inside the caller's workspace (same un-spoofable rule as `federation.query`'s `{source}`). The
query-builder workbench (scope in `docs/scope/frontend/query-builder/`) builds on this same wiring.

## Federate vs mirror (both blessed by `0003`)

- **Federate** (`federation.query`) — read the external DB live, for fresh/ad-hoc/interactive needs.
- **Mirror** (`federation.mirror`) — a durable `lb-jobs` batch that copies a range into the series
  plane for dashboards/cache/offline; resumes mid-range on restart (the job cursor + ingest dedup
  `(series, producer, seq)`), never double-writing. SurrealDB stays authority either way.

## Tests (the gate — all green, against a REAL spawned database)

The external DB is the one sanctioned fake-boundary (testing §0), behind the single `Source` trait —
tests run against a **real spawned Postgres** (`postgres:16-alpine` via docker; a SQLite-file source is
the documented fallback). Categories: capability-deny (incl. the **`net:*` deny**), workspace-
isolation, SELECT-only enforcement, the `add → test → query` round-trip on seeded rows, secret
redaction, **CRUD-across-two-admins** without owner-wall denial (+ the stale-owner heal), and
**mirror-resumes-mid-range** without double-writing. The real-Postgres e2e + the federation validator
(7), host net/validate (7), the datasource-CRUD-ownership host suite (4), `lb-secrets` (14, incl. the
`reclaim` owner-heal), and `ext-loader` grant (12) unit suites all pass. The **sqlite e2e**
(`host/tests/federation_sqlite_test.rs`, needs NO Docker) covers the same mandatory categories for
`kind:"sqlite"`: probe/schema/query against a real seeded `.db`, the honest missing-path error (no
empty-file creation), path-DSN redaction, capability-deny, and workspace-isolation; the UI add-form
kind select + sqlite add are covered in `DatasourcesAdmin.gateway.test.tsx` (real gateway).
