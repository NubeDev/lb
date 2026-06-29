# Datasources / federation — end-to-end verification + toolchain portability fix

Status: session log. The `federation` extension (datasources scope) was already shipped; this session
re-verified it works end to end on a fresh box and removed a hardcoded toolchain assumption that
blocked the Postgres E2E from building on a machine with a normal system toolchain.

## What was asked
"Get the federation extension working end to end — show that we can add a new datasource."

## What was already in place (confirmed, not rebuilt)
- Child sidecar (`rust/extensions/federation/`): control protocol over framed stdio, `federation.query`
  + `datasource.test`, DataFusion engine, per-table providers, SELECT-only validator, sqlite +
  (feature-gated) postgres `Source` impls.
- Host service (`rust/crates/host/src/federation/`): the `datasource.add/remove/list/test`,
  `federation.query`, `federation.mirror` verbs with workspace-first auth, `net:*` pre-connect
  enforcement, DSN secret-mediation, MCP bridge dispatch.
- Gateway routes (`role/gateway/src/routes/datasources.rs`) + tests.

## What ran green
- `cargo test -p federation` — 7 validator unit tests.
- `cargo test -p lb-role-gateway --test datasources_routes_test` — 5 (add→list round-trip, per-verb
  cap-deny, two-session workspace isolation, DSN redaction, honest-red probe with no sidecar).
- `cargo test -p lb-host --test federation_test` — the headline E2E against a **real spawned
  `postgres:16-alpine` container** seeded with real rows:
  - register a new datasource (`datasource.add` with DSN → `lb-secrets`, ref-only in the record),
  - `datasource.list` shows the source, DSN redacted (`secret_ref` only),
  - `datasource.test` → green (real connectivity probe through the supervised sidecar),
  - `federation.query` returns the 5 seeded rows live,
  - SELECT-only enforced (INSERT/DROP/UPDATE rejected as `BadInput`),
  - capability-deny (`federation.query` w/o cap; `datasource.add` w/o admin cap → opaque `Denied`),
  - workspace isolation (ws-B cannot resolve ws-A's source),
  - `net:*` deny (a source whose endpoint the install grant omits → opaque refusal even installed),
  - resumable mirror (range=3 → 3 series rows; resume range=5 → 5, never 8 — no double-write).

```
running 1 test
federation test DB path: REAL Postgres (postgres:16-alpine) on 127.0.0.1:49195
test federation_end_to_end_postgres ... ok
test result: ok. 1 passed; 0 failed; 0 ignored
```

## Fix made
`crates/host/tests/federation_test.rs::federation_dir()` unconditionally set
`RANLIB=/home/user/.local/bin/zigranlib` for the postgres feature build. That path is environment-
specific (a zig wrapper for boxes lacking a system toolchain); on a machine with a normal system
`ranlib`/`cc`/`perl`, forcing a nonexistent `RANLIB` breaks the vendored-OpenSSL build. Changed to
only override `RANLIB` **when the zig wrapper actually exists**, so the default system toolchain is
used otherwise. Verified the E2E now builds and passes via the test's own build path (no
`FEDERATION_BIN` escape hatch needed).

## Notes for next time
- The Postgres E2E build is heavy (~2.5 min cold: vendored OpenSSL + datafusion-table-providers
  postgres connector). The binary is ~330 MB in `target/debug/federation`.
- To skip the rebuild when iterating, the test honors `FEDERATION_BIN=<path to a built federation>`.
- This box has a full system toolchain (`cc`/`gcc`/`clang`/`perl`/`make`/`ranlib`), so the postgres
  feature compiles natively without the zig wrapper.
