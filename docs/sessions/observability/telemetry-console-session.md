# Observability — the telemetry console (session)

- Date: 2026-06-30
- Scope: ../../scope/observability/telemetry-console-scope.md (+ ../../scope/observability/observability-scope.md for the emit half)
- Stage: S10 — cross-cutting retrofit (the consumer half of observability)
- Status: done

## Goal

Build the telemetry console end to end: a reusable capped-retention store primitive, a
`tracing-subscriber` Layer that writes the redacted event schema to a FIFO-capped `telemetry`
table, the gated + workspace-walled MCP read surface (`telemetry.query`/`trace`/`tail`/`purge`),
the SSE tail route, and the in-browser console with filters + a live tail + an audit lane.
Exit criterion: the capped primitive is provably correct under concurrency, the read surface is
workspace-walled and capability-gated, secrets never reach the ring, and the console reads it all
over a real spawned gateway — all proven by real tests (no fakes, CLAUDE §9).

## What changed

Much of the backend was already drafted by a parallel session; this session **hardened the
load-bearing pieces to correctness, built the seed path, and built the entire UI + its tests.**

Backend (Rust):
- `rust/crates/store/src/capped.rs` — `capped_insert` made **deterministically correct under
  concurrency**: per-`(ns,table,cap_key)` in-process async lock (`key_lock`) + a bounded
  **retry-on-conflict** loop (`MAX_CONFLICT_RETRIES`, escalating backoff). The single
  insert+trim transaction stays the unit; the lock removes the snapshot-interleaving that
  over-evicted, the retry handles the cross-key/reader conflict `kv-mem` raises (which the
  fire-and-forget Layer would otherwise silently drop → under-count).
- `rust/crates/telemetry/src/layer.rs` — the Layer now **filters to `target == lb.telemetry`**
  so it stops capturing SurrealDB's own internal `tracing` events (which `capped_insert`'s
  queries emit — the sink was recursively logging its own writes). Extracted the write body into
  `write_event` + the collection into `collect_event` (both `pub`) so a test can drive the REAL
  collection (redaction) + write path **deterministically** instead of racing the spawn.
- `rust/crates/host/src/telemetry/seed.rs` (new) — `telemetry_seed`: the real-path seed
  (`capped_insert` + tail publish) the test-gateway uses; exported from `lb_host`.
- `rust/role/gateway/src/bin/test_gateway_seed.rs` — `POST /_seed/telemetry` (real write path).
- `rust/role/gateway/src/session/credentials.rs` — `member_caps` grants `mcp:telemetry.read:call`
  (NOT `audit.query`, so the dev session exercises the degraded audit lane).
- `rust/crates/host/Cargo.toml` — `tracing` moved to `[dependencies]` (was dev-only; the host
  now build-depends on it for the dispatch instrumentation).
- Also added the missing `pub mod watch;` in `flows/mod.rs` to unblock the shared workspace build
  (a one-line declaration for a file the parallel flows session had left undeclared — see Dead ends).

Frontend (React/TS):
- `ui/src/lib/telemetry/` — `telemetry.types.ts`, `telemetry.api.ts` (query/trace/purge over the
  `mcp_call` bridge), `telemetry.stream.ts` (SSE tail), barrel.
- `ui/src/features/telemetry/` — `useTelemetry.ts` (snapshot + live fold + trace pivot),
  `filterUrl.ts` (shareable URL codec), `TelemetryFilterBar.tsx`, `TelemetryList.tsx`,
  `AuditLane.tsx` (degraded labelled state — audit unshipped), `TelemetryView.tsx`, barrel.
- Shell wiring: `NavRail.tsx` (Telemetry surface + Telescope icon), `routing/surface.ts`,
  `routing/allowed.ts` (gated on `telemetryRead`), `routing/createAppRouter.tsx` (the route).
- `ui/src/lib/session/admin-caps.ts` — `telemetryRead` + `auditQuery` cap constants.
- `ui/src/test/gateway-session.ts` — `seedTelemetry` helper.

## Decisions & alternatives

- **Capped correctness = single-tx + per-key lock + retry-on-conflict** (not any one alone). The
  scope mandated the single transaction; testing showed that is necessary but **not sufficient**
  under `kv-mem` optimistic MVCC. Rejected: count-then-delete (races to over-evict); reaper-only
  (overshoots in a burst); retry-only (no conflict is raised for the same-key over-evict shape);
  a native SurrealDB ring (doesn't exist — count-bounded FIFO is why we own the primitive). See
  [debugging](../../debugging/observability/capped-insert-overgrows-cap-under-concurrency.md).
- **The Layer must filter by `target`.** Without it the ring fills with SurrealDB's internal logs
  and the sink recursively logs its own `capped_insert` queries. Only `lb.telemetry` dispatch
  events belong in the bounded console ring; everything else is stderr/OTLP's job.
- **No `telemetry.write` tool; deny is capability-first.** Writes come from the Layer only. An
  ungranted `telemetry.tail` returns opaque `Denied` (the auth gate fires before the bridge),
  NOT `NotFound` — the deny path must not leak the verb's SSE shape. A *granted* bridge call to
  `telemetry.tail` returns `NotFound` (the feed rides SSE) — both asserted.
- **Audit lane degrades, never fakes.** Audit hasn't shipped (`audit.query` doesn't exist), so the
  lane renders a labelled "unavailable / needs-grant" state — never telemetry rows masquerading as
  the mutation ledger (the two-store guarantee). The dev session deliberately lacks `audit.query`.
- **Reads over the generic `mcp_call` bridge, tail over its own SSE route.** No new gateway route
  for the snapshot reads (the bridge already exists); the tail gets its own
  `routes/telemetry_stream.rs` (one responsibility per file), as the scope resolved.
- **Deterministic test harness.** Replaced the flaky global-subscriber + fire-and-forget-spawn
  harness with: a leaked harness runtime (so the shared store/bus outlive any one test), a shared
  bootstrapped `Node` (so four `Node::boot()`s don't flood Zenoh discovery), and a capturing layer
  that drives `collect_event` then **awaits** `write_event` (so a following read sees the row).

## Tests

Mandatory categories — all against the REAL `mem://` store + real bus + real gateway, no fakes:

- **Capability-deny per verb** — `telemetry.query`/`trace`/`tail` denied (opaque) without
  `telemetry:read`; UI deny test for a session holding only `inbox.list`.
- **Workspace-isolation** — ws-B query returns ONLY ws-B rows (backend host test + UI gateway test).
- **FIFO-cap (the headline primitive)** — `cap+k` for one key → exactly `cap`, newest survive;
  per-source key doesn't evict a quiet source; global key bounds across sources (same helper).
- **Concurrency** — 100 concurrent `capped_insert`s at 5× cap → final count EXACTLY `cap`
  (the test that proves the single-tx + lock + retry design). Now deterministic (10/10).
- **Redaction** — a planted secret through a tool param reaches the ring as `params_digest` only;
  appears in ZERO stored rows and ZERO query output (driven through the real Layer collection).
- **Filter narrowing + trace correlation** — source/level/text narrow; `telemetry.trace` correlates.
- **Unified-view provenance** — the audit lane is a separate, labelled store, never the ring
  (degraded empty state asserted by construction; no fake rows).
- **SSE live tail (UI)** — over the real spawned gateway: open the tail, seed a row, assert it
  arrives as a live frame (read via fetch+reader; jsdom has no EventSource).

Green output:

```
# rust/crates/store
test result: ok. 6 passed; 0 failed   (capped_test, run 3× — deterministic)

# rust/crates/telemetry
test result: ok. 4 passed; 0 failed   (lib: secret + redact)
test result: ok. 1 passed; 0 failed   (layer_spike)

# rust/crates/host (telemetry_test)
running 7 tests
test filters_narrow_and_trace_correlates ... ok
test query_returns_only_callers_workspace_rows ... ok
test tail_via_bridge_is_notfound_when_granted ... ok
test query_denied_without_read_cap ... ok
test trace_denied_without_read_cap ... ok
test planted_secret_appears_in_zero_stored_rows_or_query_output ... ok
test tail_denied_without_read_cap ... ok
test result: ok. 7 passed; 0 failed

# rust/role/gateway (whole crate)
test result: ok. 3 / 7 / 2 / 8 / 9 / 5 / 5 / 3 / 13 passed; 0 failed

# cargo build --workspace : Finished   |   cargo fmt --check : clean

# ui unit (vite.config.ts)
Test Files  27 passed (27)   Tests  186 passed (186)   (incl. filterUrl.test.ts: 6)

# ui gateway (vitest.gateway.config.ts) — telemetry slice
✓ src/features/telemetry/TelemetryView.gateway.test.ts (4 tests)
```

## Debugging

- [observability/capped-insert-overgrows-cap-under-concurrency.md](../../debugging/observability/capped-insert-overgrows-cap-under-concurrency.md)
  — the capped over-count (snapshot interleaving) AND the under-count (dropped retryable conflict);
  regression test `concurrent_inserts_past_cap_leave_exactly_cap`, now 10/10.

## Public / scope updates

- Promoted shipped truth to `public/observability/observability.md` (the console section).
- Resolved scope open questions (see that doc's "Resolved" block): default caps (1000/source,
  config-overridable), per-source key default, ULID insert-seq, `capped` lives in `lb-store`,
  `telemetry.tail` has its own route, v1 reads the local ring. Trim cadence: strict (every insert)
  for v1 — amortized + slack is a documented deferral.

## Dead ends / surprises

- **The capped flakiness was two distinct bugs, not one.** First the snapshot-interleaving
  over-count (fixed by the per-key lock), then — only once the Layer drove it under a real
  telemetry test — a *retryable conflict* the fire-and-forget Layer silently swallowed, producing
  an *under*-count. The complete fix is lock + retry; either alone is insufficient.
- **The Layer was logging SurrealDB's own events** (and its own writes' queries) — invisible until
  a raw row dump showed "Parsing SurrealQL query" rows. The `target` filter fixes it.
- **The test harness, not the product, was most of the flakiness.** Global-default subscriber races
  and fire-and-forget spawns that didn't land in the poll window. The capture-then-await harness is
  deterministic and still exercises the real collection + write path.
- **A parallel "flows" session was editing the same workspace.** Its in-progress edits broke the
  shared `cargo build` twice (an undeclared `watch` module; `tracing` in the wrong dependency
  section; a `!Send` async-recursion). I added the minimal unblocking declarations where they were
  clearly the other session's intent and matched my own need (`tracing` as a real host dep), and
  otherwise stayed out of their files. We coexisted: both sessions' debug entries sit side by side.

## Follow-ups

- The wider `pnpm test:gateway` suite has pre-existing **flaky** failures unrelated to telemetry
  (`SystemView`, `ProofPanel`, `App` routing — different tests fail run to run; the known
  many-in-process-peers flake class). The telemetry slice is consistently green. Worth a separate
  stabilization pass (serialize the gateway suite or isolate peers).
- Amortized trim cadence (every *m* inserts, a tested slack bound) — deferred; v1 trims every insert.
- Cross-node console reads (operator debugging an edge from the hub) — deferred to a routed
  `telemetry.query`, per the scope.
- The cross-tenant operator console (a separate, higher `telemetry:read:all` capability) — not built.
- STATUS.md updated (S10 slice state).
```
