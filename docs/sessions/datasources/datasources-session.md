# Datasources / federation extension — session

- Date: 2026-06-28
- Scope: ../../scope/datasources/datasources-scope.md (+ the rules sibling,
  ../rules/rules-session.md — a rule reaches a datasource via `federation.query`)
- Stage: post-S8 platform capability (federation plane); see STATUS.md
- Status: done (federation extension + host wiring shipped; real-Postgres e2e green)

## Goal

A workspace connects to **external SQL sources** (MySQL/Postgres/Timescale, the DataFusion-federated
set) and queries them from rules/agent/UI/extensions — **without breaking "one datastore" (rule 2) or
the workspace wall (rule 6)**. The answer (per `0003` + the reference-extensions doctrine): a **native
(Tier-2) `federation` extension** that embeds DataFusion + connectors as a *library*, owns the pools
behind one `Source` trait, `net:*` + secret-gated, and exposes one read-first verb `federation.query`
+ datasource CRUD + a `federation.mirror` job. SurrealDB stays authority; external DBs are federated
sources reached only through the gated extension.

## Platform prerequisites landed this session (shared with rules)

- **`caps` `Net` surface + `Connect` action** — the grammar addition the scope names
  (`net:tls/host/port:connect`). The supervisor enforces `requested ∩ admin_approved` at connect.
- **`lb-secrets` capability-mediated store** — `secret:federation/{source}` holds the DSN, pulled by
  the host/supervisor and handed to the pool, never returned to a caller, a record, or a log.

## What changed

The full federation build — the native sidecar `extensions/federation/`, the host
`crates/host/src/federation/` service, the `datasource:{ws}:{name}` record, and the dispatch wiring —
is logged in detail in the sibling **`federation-session.md`** (same directory). Summary:

- **Sidecar** (`extensions/federation/`): a JSON-RPC stdio binary reusing `lb-supervisor`'s wire
  protocol (like echo-sidecar), embedding `datafusion` 53 + `datafusion-table-providers` 0.11 as a
  LIBRARY behind one `Source` trait (one impl/kind: postgres, sqlite). SELECT-only validator via
  `datafusion::sql::sqlparser`. Heavy deps live ONLY here (core links no DB driver, rule 2/1).
- **Host service** (`crates/host/src/federation/`, one verb/file): `federation.query` (resolve source
  in the caller's ws → `net:*` pre-connect → mediate the DSN out of `lb-secrets` → route to the
  supervised sidecar → SELECT-only re-validate), `federation.mirror` (a durable `lb-jobs` batch into
  the series plane, resumes mid-range), `datasource.add`/`remove`/`list`/`test`. The
  `datasource:{ws}:{name}` record holds kind + endpoint + secret REF, **never the DSN**.
- **`ext-loader::grant`** gained per-endpoint `net:*` intersection so an admin-approved concrete
  endpoint is granted against a manifest's wildcard request (`requested ∩ admin_approved`).
- Wired into `tool_call.rs` + `lib.rs` (merged with the rules/chains dispatch from this same session).

## Doctrine held

- **SurrealDB is NEVER a DataFusion source** (rule 2) — platform data is read natively; only external
  DBs are federated.
- **Workspace-pinned at the host** — `federation.query`'s `{source}` resolves only to a
  `datasource:{ws}:{name}` in the caller's workspace; a caller can't name a cross-tenant source.
- **SELECT-only validated** — a write/DDL is rejected before execution (v1 read-first); a
  `federation.write` is a separate, later, Ask-gated verb.
- **Federate vs mirror** — `federation.query` reads live; `federation.mirror` is a durable `lb-jobs`
  batch that `ingest.write`s a range into the platform series plane (resumes mid-range on restart).

## Tests (all green — against a REAL spawned database)

DB path used: **real Postgres** (`postgres:16-alpine` via `docker run`, random port, seeded with real
rows). A SQLite-file `Source` is the documented fallback where the Postgres TLS connector can't build.
The DB is the one sanctioned fake-boundary (testing §0), behind the single `Source` trait.

Mandatory categories covered: capability-deny (`federation.query` without cap; `datasource.add`
without admin cap; the **`net:*` deny** — a source whose endpoint the grant omits → opaque refusal
even when installed); workspace-isolation (ws-B cannot resolve/query a ws-A source); SELECT-only
(INSERT/UPDATE/DDL rejected host-side before the sidecar); the happy round-trip (`add` → `test` green
→ `query` returns the seeded rows); secret mediation (the DSN is absent from `datasource.list` and
from a query result — a redaction assertion); and **mirror resumes mid-range** after a restart without
double-writing (the `lb-jobs` cursor + ingest `(series,producer,seq)` dedup).

```
$ cargo test -p lb-host --test federation_test
test federation_end_to_end_postgres ... ok
test result: ok. 1 passed; 0 failed
$ cargo test -p federation -p lb-secrets -p lb-ext-loader   # 7 + 3 + 12 unit -> all green
$ cargo test -p lb-host --lib federation                    # 7 host net/validate unit -> green
$ cargo build --workspace                                   # Finished — green
```
