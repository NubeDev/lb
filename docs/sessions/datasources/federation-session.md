# Session — `federation` native (Tier-2) datasources extension

Scope: `docs/scope/datasources/datasources-scope.md`. Built the `federation` extension + its host
service end to end, tested against a **real spawned Postgres** container.

## What shipped

### The native sidecar — `rust/extensions/federation/`
- `extension.toml` — `[native]` block (exec `federation`, restart `on-crash`), tool declarations for
  `federation.query` / `datasource.test` / `federation.mirror`, and `request = [net:tls:*:*:connect,
  secret:federation/*:get]`.
- `src/main.rs` — the JSON-RPC stdio loop reusing `lb-supervisor`'s `rpc`+`frame` verbatim (same ABI
  as `echo-sidecar`); handles `Method::Call` for `federation.query {kind,dsn,source,sql}` and
  `datasource.test {kind,dsn}`.
- `src/source/{mod,postgres,sqlite}.rs` — the ONE `Source` trait (testing §0 boundary), one impl per
  kind. Postgres owns a `PostgresConnectionPool` behind the trait; the DSN is handed in by the host
  per call and lives only inside the pool (never stored on the struct, never logged).
- `src/validate.rs` — SELECT-only validator using `datafusion::sql::sqlparser` (ported idea from
  rubix-cube): single statement, must be a read query, collects referenced table names; rejects
  INSERT/UPDATE/DELETE/DDL/multi-statement.
- `src/query.rs` — registers each referenced table as a DataFusion `TableProvider`, runs the SQL in a
  `SessionContext`, caps rows, returns `{columns, rows}`.
- Embeds `datafusion` 53 + `datafusion-table-providers` 0.11 + arrow as a LIBRARY — only in this
  crate's Cargo.toml (core links no DB driver). Crate-level MIT/Apache-2.0 attribution to rubix-cube.
- The Postgres connector pulls native-tls→openssl; gated behind an opt-in `postgres` feature
  (`--features postgres`, vendored OpenSSL). Default build = sqlite only (the documented fallback).

### The host service — `rust/crates/host/src/federation/` (one verb per file)
- `record.rs` — the `datasource:{ws}:{name}` store record (kind + endpoint + secret REF; never the
  DSN). `authorize.rs` — the `mcp:<verb>:call` gate. `net.rs` — pre-connect `net:*` enforcement
  against the federation install grant (dot-safe colon matching). `secret.rs` — DSN mediation out of
  `lb-secrets` under the extension's OWN grant. `validate.rs` — host-side SELECT-only pre-check.
- `add.rs`/`remove.rs`/`list.rs`/`test.rs`/`query.rs`/`mirror.rs` — the verbs. `query` resolves the
  source in the caller's ws (un-spoofable), re-validates, enforces net, mediates the DSN, routes to
  the supervised sidecar. `mirror` enqueues a durable `lb-jobs` batch (reuses `lb-jobs`, checkpoints
  the cursor, resumes mid-range; ingest dedup `(series,producer,seq)` prevents double-write).
- `tool.rs` — the `federation.*`/`datasource.*` MCP bridge (`call_federation_tool`).
- Wired into `tool_call.rs` (`is_host_native` + a dispatch branch) and `lib.rs` (`pub use`).

### Supporting changes
- `crates/caps/src/request.rs` — added `Surface::Net` + `Action::Connect` (the prompt said these were
  added in the main session's checkout; this worktree branch lacked them, so added here — may merge-
  conflict, which is expected/fine).
- `crates/secrets/src/lib.rs` — implemented the S0-placeholder secrets crate (`get`/`set`, capability
  + workspace gated). This branch had only the stub.
- `crates/ext-loader/src/grant.rs` — `grant()` now does **per-endpoint** net intersection: a specific
  approved `net:tls:host:port:connect` is granted when a requested net wildcard covers it (the
  manifest can only request a static pattern; the admin approves the concrete endpoint). Dot-safe
  colon matching (the generic grammar splits on `.`, which would shred an IP/hostname).

## Tests — `rust/crates/host/tests/federation_test.rs`

DB path used: **REAL Postgres** (`postgres:16-alpine`, spawned via `docker run` on a random port,
seeded with real rows). Docker is available in this env; the federation sidecar is built with
`--features postgres` (vendored OpenSSL via the zig `ranlib` wrapper). A SQLite-file fallback Source
exists for environments where the Postgres connector cannot compile/run.

Categories covered (all green): capability-deny (`federation.query` without cap; `datasource.add`
without admin cap; the `net:*` deny — a source whose endpoint the grant omits → opaque refusal even
installed); workspace-isolation (ws-B cannot resolve/query a ws-A source); SELECT-only (INSERT/DDL/
UPDATE rejected); happy round-trip (`add` → `test` green → `query` returns 5 seeded rows); secret
mediation (DSN absent from `datasource.list` and from a query result — redaction asserts); mirror
resumes mid-range after restart and does not double-write (3 then resume → 5, never 8).

```
running 1 test
test federation_end_to_end_postgres ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.80s
```

Plus unit tests: federation validator (7), host federation net/validate (7), secrets (3),
ext-loader grant (12). Full `cargo build --workspace` green.

## Decisions
- DSN flows host→child in the per-call input (not via spawn env), so a stateless restart needs no
  re-handshake to carry secrets and the value never touches a record/log/result.
- Net matching is a dedicated dot-safe colon matcher (not the generic caps grammar) because hostnames
  contain dots and the grammar splits resources on `.`.
- Postgres connector is feature-gated so the crate (and tests of the sqlite path) build with no TLS
  toolchain; the headline Postgres path is opt-in where OpenSSL can be built.
