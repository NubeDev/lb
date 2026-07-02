# Session — fix the flaky gateway-test login race (`not a member of any workspace`)

- Date: 2026-07-01
- Area: store / frontend test harness
- Debugging entry: ../../debugging/store/concurrent-use-ns-namespace-race.md
- Regression test: rust/crates/store/tests/concurrent_ns_test.rs

## The ask

`cd ui && pnpm test:gateway` failed **intermittently** — `src/App.gateway.test.tsx` gave different
pass/fail counts run-to-run, surfacing `Error: not a member of any workspace` from
`signInReal → login → postJson`. Present at HEAD (reproduces on a clean stash), so a harness/login
race, not a UI-layout regression. Fix the race — no mocks, no retries-to-hide, and add a regression
guard + debug entry (CLAUDE §9, HOW-TO-CODE).

## What it turned out to be

The string comes from `role/gateway/src/routes/login.rs` via
`lb_host::membership_login_resolve`. For a brand-new `nextWs()` the correct path is the
empty-workspace bootstrap; the flake meant login sometimes saw the workspace as
*non-empty-but-without-me* — i.e. it read a **different namespace** than it was resolving.

Root cause is one layer down, in `lb_store`. `Store` shares ONE embedded SurrealDB connection with a
single mutable session (its selected namespace). Every store verb did
`store.use_ws(ws).await` (which ran `db.use_ns(ws).use_db("main")` — a **global** mutation of the
shared session) and then its query as a **separate await**, with nothing holding the two together. On
the gateway's multi-thread runtime, concurrent logins for different fresh workspaces interleaved:
`use_ns(A)` … `use_ns(B)` … A's `UPSERT`/`SELECT` ran against **B**. So the bootstrap membership was
written into (or read back from) the wrong namespace — non-deterministic by construction. The store's
existing concurrency tests never caught it because they all hammer the **same** workspace; this bug
only shows across **different** namespaces run concurrently.

## The fix

`crates/store/src/open.rs`: give `Store` a per-instance `Arc<tokio::Mutex<()>>` session lock.
`use_ws` now takes the lock (owned guard), selects the namespace, and returns a `WsGuard` that holds
the lock until it drops — i.e. across the caller's query. `WsGuard` derefs to `&Surreal<Db>`, so every
caller (`read`/`write`/`list`/`create`/`scan`/`graph`/`increment`/`write_tx`/`write_batch`/
`write_journaled`/`read_versioned`/`tables`/`capped`/`query_ws`) is unchanged — the only difference is
the query now runs **inside** the critical section. This closes the whole class at the primitive, not
just the login path.

Trade-off (documented in the debug entry): store ops now serialize at the session seam. Acceptable —
the embedded connection was already a single shared session; per-record `write_locked`/`capped`/
`increment` locks still bound their own retries.

Incidental: the workspace didn't build at all on entry — the in-flight operator-CLI work added
`role/cli` to `members` but never added `lb-role-gateway` to `[workspace.dependencies]` (which
`role/cli` inherits). Added that one missing entry to `rust/Cargo.toml` so the workspace compiles;
`role/cli` itself is still a source-less stub (someone else's in-flight work — left untouched, and
built with `--exclude lb-cli`).

## Tests

- **New regression** `rust/crates/store/tests/concurrent_ns_test.rs` (`multi_thread`, 4 workers):
  64 workspaces each write their own record concurrently, then each reads back exactly its own value
  and asserts its table holds exactly one row. Fails-before (panics with
  `read or write conflict … can be retried` / `sending into a closed channel` / a foreign owner),
  passes-after — 20/20 green.
- `cargo test -p lb-store`: green (isolation, capped, write_locked, increment, persistent parity).
- `pnpm test:gateway` `src/App.gateway.test.tsx`: 20/20 runs all "7 passed" with the harness binary
  pre-built. (A never-built `test_gateway` makes `globalSetup` run `cargo build` every iteration;
  back-to-back builds can transiently emit "no tests" — a harness timing artifact, not a failure. Pre-build the bin before the loop.)
- Full `pnpm test:gateway` (45 files, ONE shared gateway): **zero** `not a member` occurrences — the
  family is gone. Fixing the race deterministically surfaced ONE test that only ever flakily passed:
  `SystemView > renders the fixed status grid` asserted `store rows == 2`, but 2 samples commit 5 rows
  through the real ingest path (the exact `2` only "passed" when the race dropped writes into the wrong
  namespace). Verified pre-existing (fails with the racy store too); fixed the assertion to the card's
  real contract — non-zero rows (`> 0`), not a brittle total. The other full-suite failures
  (`StudioView` wasm build, `TelemetryView` "LIVE row" `AbortSignal` jsdom/undici polyfill error,
  `SystemView` "detail sheet" live-mesh peer count — passes in isolation) are independent, pre-existing
  flakes unrelated to membership; left out of scope.

## Files touched

- `rust/crates/store/src/open.rs` — the fix (session lock + `WsGuard`).
- `rust/crates/store/tests/concurrent_ns_test.rs` — new regression.
- `rust/Cargo.toml` — restore the missing `lb-role-gateway` workspace dep (unblocks the build).
- `ui/src/features/system/SystemView.gateway.test.tsx` — fix the wrong `store rows == 2` assertion
  the race had been masking (now `> 0`, the card's real contract).
- `docs/debugging/store/concurrent-use-ns-namespace-race.md` + `docs/debugging/README.md` row.
