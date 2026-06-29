# Datasources / federation — wiring the running node so the Datasources page works live

Status: session log. The `federation` extension + host service + gateway routes were already shipped
and green in tests, but the **running dev node never installed the federation sidecar**, so the live
Datasources page showed "No datasources yet" and Test/query had no sidecar to reach. This session
mounts the federation role at boot and fixes two real end-to-end gaps the tests didn't catch.

## Symptom
The browser Datasources page (`/t/acme/datasources`) showed "No datasources yet" and Add/Test did
nothing useful — even though `make dev` was running and a seeded TimescaleDB was up on `:5433`.

## Root causes (two real gaps, each invisible to the existing tests)
1. **The node never installed the federation extension.** `node/src/main.rs` loaded `hello` and the
   enabled-wasm set, mounted the github role by env — but nothing installed the native `federation`
   sidecar or approved a `net:*` endpoint. So `datasource.add`/`list` worked (store-only) but
   `datasource.test`/`federation.query` had no sidecar and no grant.
2. **The dev login lacked `mcp:native.call:call`.** `federation.query`/`datasource.test` dispatch to
   the sidecar through `call_sidecar`, which gates on `mcp:native.call:call`. The dev login granted
   `mcp:native.install:call` but NOT `…call:call`, so the live page got an opaque `denied` (HTTP 500,
   body "denied") at the sidecar hop — *after* a green Add/list. The route-level cap test passed
   because it never reaches the sidecar.

## Changes
- **`rust/node/src/federation.rs` (new)** — an env-gated role mount (the `github.rs` pattern, §3.1, no
  `if cloud`). `LB_FEDERATION_ENDPOINTS=host:port,…` installs + supervises the `federation` sidecar in
  `LB_WORKSPACE` with `net:tls:host:port:connect` per endpoint + `secret:federation/*:get`. Optional
  `LB_FEDERATION_SEED_{NAME,KIND,ENDPOINT,DSN}` pre-registers one source (DSN → `lb-secrets`, ref-only
  on the record) so the page has a working entry on first boot. Mounted from `main.rs`.
- **`rust/node/Cargo.toml`** — add `lb-supervisor` (for `OsLauncher`).
- **`rust/role/gateway/src/session/credentials.rs`** — add `mcp:native.call:call` to the dev login,
  plus a regression unit test (`dev_login_carries_the_full_datasources_chain`) asserting the full
  add→test→query cap chain is present.
- **`Makefile`** — `FED_ENDPOINTS`/`FED_SEED_*` vars (default the dev TimescaleDB on `:5433`); `make
  dev`/`make cloud` now build the postgres-featured sidecar (new `federation` target) and inject the
  federation env. Clear `FED_ENDPOINTS=` to disable.
- **`rust/crates/host/tests/federation_test.rs`** — only set the zig `RANLIB` wrapper when it actually
  exists (this box has a system toolchain); see the prior verify session.

## Proof — live, against the real seeded TimescaleDB (475 984 readings)
Ran the built node with the federation env on `:7799` against the running `lb-timescaledb` container,
then drove the gateway exactly as the UI does:

```
federation: installed sidecar in 'acme' (tools=["federation.query","datasource.test","federation.mirror"],
  granted=["secret:federation/*:get","net:tls:127.0.0.1:5433:connect"], approved endpoints=["127.0.0.1:5433"])
federation: seeded datasource 'timescale' (postgres @ 127.0.0.1:5433) in 'acme'

GET  /datasources                       → {"datasources":[{"name":"timescale","kind":"postgres",
                                            "endpoint":"127.0.0.1:5433","secret_ref":"federation/timescale"}]}
POST /datasources/timescale/test        → {"ok":true}  HTTP 200   (real probe through the sidecar)
POST /mcp/call federation.query sites   → {"columns":["id","name"],"rows":[["site-001","Northside Factory"],…]}
POST /mcp/call federation.query JOIN    → per-site reading counts: 210252 / 125564 / 140168
```

## Tests
- `lb-role-gateway` datasources routes — 5 green; new credentials regression — green.
- `lb-host` federation E2E (real spawned Postgres) — green.
- `cargo build --workspace` + `cargo fmt` — clean.

## Third gap — dashboard table widget showed no data (columnar frame conversion)
A table widget bound to the `timescale` source dispatches `federation.query` THROUGH `viz.query`,
whose `result_to_rows` (`crates/host/src/viz/frame.rs`) turns a tool result into frame rows. It
matched `federation.query`'s `{columns:[…], rows:[[…], …]}` on the generic `rows` key and passed the
**column-aligned arrays straight through** — so the frame got empty fields and the widget rendered
`length: 3` rows of `{}` (no data). Fixed `result_to_rows` to detect the columnar `{columns, rows}`
shape (rows are arrays) and zip each row against `columns` into a named object; object-shaped `rows`
(store.query/series) still pass through unchanged. Unit tests in `frame.rs` + a real-DB assertion in
`federation_test.rs` (`viz.query` over a federation target returns named fields + 5 rows) lock it in.
The existing `viz_query_test` only covered the *deny/empty* federation path, which is why this slipped.

Proof (live, dev gateway :8080, real TimescaleDB):
```
viz.query {sources:[{tool:"federation.query", args:{source:"timescale", sql:"SELECT * FROM site"}}]}
→ rows: [{id:"site-001", name:"Northside Factory"}, {id:"site-002", …}, {id:"site-003", …}]
```

## Gotcha — the sidecar binary path is shared across feature sets
`cargo build --workspace` builds `federation` **without** `--features postgres` into the SAME
`target/debug/federation` the postgres build uses, silently replacing it. The sidecar then reports an
honest `source error: postgres source not built in (rebuild federation with --features postgres)` and
Test/query 500. Fix: rebuild with the feature (`make federation` or
`cargo build -p federation --features postgres`) AFTER any bare `--workspace` build, and restart the
node so the supervisor spawns the fresh binary. (A future hardening: build the postgres sidecar to a
distinct artifact name so a no-feature workspace build can't clobber it.)

## How to use
`make dev` (or `make cloud`) now brings up the federation sidecar + seeds `timescale` against
`docker/postgres` automatically. Bring the DB up first: `cd docker/postgres && docker compose up -d`
then `./seed.sh`. Override the target DB with `make dev FED_SEED_DSN="host=… port=… …"`, or disable
federation entirely with `make dev FED_ENDPOINTS=`.
