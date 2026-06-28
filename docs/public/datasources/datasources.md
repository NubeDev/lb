# Datasources (public)

Status: **SHIPPED** (2026-06-28). Scope: `../../scope/datasources/datasources-scope.md`. Sessions:
`../../sessions/datasources/datasources-session.md` + `../../sessions/datasources/federation-session.md`.
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
- **SELECT-only** — a write/DDL is rejected (read-first v1) both host-side and in the sidecar
  validator; a `federation.write` is a separate, later, Ask-gated verb.
- **Workspace wall** — `datasource:{ws}:{name}` is workspace-keyed; ws-B can neither name nor reach a
  ws-A source, and a mirror job's callback `ws` is host-set.

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
redaction, and **mirror-resumes-mid-range** without double-writing. The real-Postgres e2e + the
federation validator (7), host net/validate (7), `lb-secrets` (3), and `ext-loader` grant (12) unit
suites all pass; `cargo build --workspace` is green.
