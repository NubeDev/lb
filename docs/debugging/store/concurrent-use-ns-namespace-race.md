# Login flakes `not a member of any workspace` — concurrent `use_ns` races the shared session namespace

- Area: store
- Status: resolved
- First seen: 2026-07-01
- Resolved: 2026-07-01
- Session: ../../sessions/frontend/signin-not-a-member-seeding-race-session.md
- Regression test: rust/crates/store/tests/concurrent_ns_test.rs

## Symptom

`cd ui && pnpm test:gateway` fails **intermittently** (varies run-to-run). The clearest repro is
`src/App.gateway.test.tsx` — e.g. "does not take workspace from a pasted URL" and the cap-denied
admin route. The error is:

```
Error: not a member of any workspace
 ❯ postJson src/lib/ipc/http.ts:691
 ❯ Module.signInReal src/test/gateway-session.ts:21
```

Re-running the same file 3× gives different pass/fail counts. It reproduces on a clean `git stash` of
the working tree (present at HEAD), so it is **not** a recent UI-layout change — it is in the
login + membership path.

## Reproduce

```bash
cd ui
for i in $(seq 1 20); do pnpm exec vitest run --config vitest.gateway.config.ts src/App.gateway.test.tsx 2>&1 | grep -E "^ +Tests "; done
```

Before the fix: a mix of "7 passed" and "N failed". Each test uses a fresh unique workspace
(`nextWs()`), so many brand-new workspaces log in against the ONE spawned gateway, concurrently.

Store-level (deterministic, no UI):
`rust/crates/store/tests/concurrent_ns_test.rs` on a `multi_thread` runtime — 64 workspaces each
write their own record concurrently, then read back. Before the fix this panics with
`read or write conflict … can be retried` / `sending into a closed channel` / a foreign owner read
back; after, 20/20 green.

## Investigation

- The exact string is emitted by `role/gateway/src/routes/login.rs:73`, from
  `lb_host::membership_login_resolve` (`crates/host/src/membership/login_resolve.rs`). It only fires
  when a workspace has members but not this sub (decision #4). For a **brand-new** `nextWs()` the
  correct path is the empty-workspace bootstrap (`has_any_effective_member == false` → bootstrap the
  requester as `workspace-admin`). So the flake means the login sometimes sees the workspace as
  *non-empty-but-without-me* — i.e. it reads a **different namespace** than the one it is resolving.
- `membership_login_resolve` does: `is_effective_member(ws)` (point read) → `has_any_effective_member(ws)`
  (table scan) → `bootstrap_first_member(ws)` (write). All three go through `lb_store` verbs.
- Every `lb_store` verb (`read`/`write`/`list`/`create`/…) begins with
  `store.use_ws(ws).await` then runs its query as a **separate await**. `use_ws` was
  `self.db.use_ns(ws).use_db("main")` on the **one** shared `Surreal<Db>` connection every `Store`
  clone holds — a *global* mutation of that connection's single session, not per-operation scoping.
- On the gateway's multi-thread runtime, two operations for different workspaces interleave:
  `use_ns(A)` … (task B) `use_ns(B)` … (task A) query runs against **B**. So a bootstrap membership
  gets written into the wrong namespace, or a membership check reads the wrong namespace — the flake,
  non-deterministic by construction. The store's own concurrency tests (capped/write_locked) never
  caught it because they all target the **same** workspace.

## Root cause

`Store` shares one embedded SurrealDB connection with a single mutable session (selected NS+DB).
Selecting the namespace (`use_ns`) and running the query it guards were two separate awaits with **no
lock between them**, so a concurrent operation for another workspace could re-point the shared session
mid-operation — a query ran against a namespace it did not select. `crates/store/src/open.rs`
`use_ws` returned `&Surreal<Db>` with no guarantee the session still pointed where the caller asked.

## Fix

`crates/store/src/open.rs`: add a per-`Store` `Arc<tokio::Mutex<()>>` session lock. `use_ws` now
acquires that lock (owned guard), selects the namespace, and returns a `WsGuard` that **holds the lock
until it drops** — i.e. across the caller's query (it `Deref`s to `&Surreal<Db>`, so callers are
unchanged). Every `use_ws → query` pair is now one critical section: only one namespace-scoped
operation touches the shared session at a time, so a query always runs against the namespace it
selected. `query_ws` inherits this (it goes through `use_ws`). Fix at the store primitive → the whole
class (every verb, every caller) is closed, not just login.

Trade-off: store operations are now serialized at the session seam. Acceptable — the embedded
connection was already a single shared session (a global point of contention), and the per-record
`write_locked`/`capped`/`increment` locks still bound their own retries. Correctness over a
theoretical throughput loss on an embedded KV.

## Verification

- `rust/crates/store/tests/concurrent_ns_test.rs`: fails-before / passes-after; 20/20 green.
- Full `cargo test -p lb-store`: green (isolation, capped, write_locked, increment, persistent parity).
- `pnpm test:gateway` `src/App.gateway.test.tsx`: 20/20 runs all "7 passed" with the harness binary
  pre-built (so `globalSetup`'s `cargo build` is a no-op — see Prevention).
- Full `pnpm test:gateway` (45 files against the ONE shared gateway — the highest-concurrency
  stressor): **zero** `not a member` occurrences. The family is gone.

### A hidden victim: `SystemView.gateway.test.tsx > renders the fixed status grid`

Fixing the race made one test that only ever *flakily* passed fail **deterministically**:
`SystemView` seeds 2 samples into a fresh workspace and asserted `store rows == 2`. But one
`writeSample` commits through the real ingest path (staging + series tables + sample rows), so the
workspace legitimately holds **5** rows. The exact `2` only "passed" when the pre-fix `use_ns` race
dropped some of those writes into the wrong namespace (undercounting to 2). Verified pre-existing:
with the racy `use_ws` reverted in, the test **also fails** (never a member issue — a wrong
expectation the race was masking). Fixed the assertion to the card's real contract — "live, non-zero
rows" (`> 0`), not a brittle fixed total (`ui/src/features/system/SystemView.gateway.test.tsx`).

Other full-suite failures seen (`StudioView` wasm build, `TelemetryView` "LIVE row" —
`RequestInit: Expected signal to be an instance of AbortSignal`, a jsdom/undici cross-realm polyfill
issue, and `SystemView` "detail sheet" — a live Zenoh peer-count that flakes only under cross-file
mesh contention, passes in isolation) are **independent, pre-existing** flakes unrelated to
membership or the store namespace, and out of scope here.

## Prevention

- Regression: `concurrent_ns_test.rs` forces the interleave on a multi-thread runtime and asserts the
  workspace wall holds per-namespace. It fails loudly if `use_ws` ever stops holding the lock across
  the query.
- Guardrail: `use_ws` returns a guard (`WsGuard`) instead of a bare `&Surreal<Db>` — the lock is now
  structurally tied to the operation's lifetime; a caller cannot query outside the critical section.
- Harness note: running the 20× loop with a stale/never-built `test_gateway` makes `globalSetup`
  invoke `cargo build` on every iteration; back-to-back builds can transiently yield "no tests"
  (globalSetup timing), which is a harness artifact, NOT a test failure. Pre-build
  (`cargo build -p lb-role-gateway --features test-harness --bin test_gateway`) before the loop.
