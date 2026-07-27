# Session — kill the `ingest.write` transaction-conflict storm (lb side: WS-A + WS-B)

- Date: 2026-07-27
- Scope (cross-repo): `rubix-ai-extensions/docs/sessions/ingest-conflict-storm/scope.md`
- This session owns the **lb** workstreams only (WS-A, WS-B). WS-C is modbus (extensions repo);
  WS-D is the rubix-ai real-node e2e that runs last against a new lb tag.
- Debugging record: `docs/debugging/ingest/2026-07-27-ingest-write-conflict-storm.md`

## Problem

A live node logged a continuous storm of `read or write conflict … can be retried` 502s from
`ingest.write` under ≥2 modbus networks pushing every 2s to one workspace. SurrealDB's abort is
transient and retryable, but nothing on the ingest path retried it, so each abort dropped a batch and
holed the raw series (and the 5-minute rollup grid built on it).

## What changed

**WS-A — bounded retry-on-conflict for `series`-table mutations (store + ingest crates).**

- `crates/store/src/conflict.rs` **(new)** — the single home for the retry primitive:
  `pub(crate) is_retryable_conflict()`, `MAX_CONFLICT_RETRIES`, `conflict_backoff()`. The
  conflict-match string now lives in exactly one function.
- `crates/store/src/lib.rs` — registered `mod conflict;`.
- `crates/store/src/{write_locked,capped,increment}.rs` — deleted their three duplicate copies of
  the matcher/const/backoff; all now call `conflict.rs`. (No behaviour change beyond `capped` now
  also treating `"Invalid revision"` as retryable, matching the other two — a harmless widening.)
- `crates/store/src/open.rs` — new `Store::query_ws_retrying`, same signature as `query_ws`, loops
  the inner query retrying only on `is_retryable_conflict`, bounded, with the shared backoff.
- `crates/ingest/src/commit.rs` — `commit_batch`'s `BEGIN…COMMIT` now uses `query_ws_retrying`, with
  the atomic+idempotent safety comment.
- `crates/ingest/src/gc.rs` — `evict_raw` now uses `query_ws_retrying`.
- `crates/ingest/src/rollup.rs` — `write_rollups` and `evict_rollups` now use `query_ws_retrying`.
- The drain `SELECT` (a read) was left on plain `query_ws`.

**WS-B — serialize drains per workspace (host crate).**

- `crates/host/src/ingest/drain_lock.rs` **(new)** — a process-wide `static` keyed-by-`ws` async
  lock (same idiom as `store::write_locked`/`increment`).
- `crates/host/src/ingest/mod.rs` — registered `mod drain_lock;`.
- `crates/host/src/ingest/drain.rs` — `drain_at_most` (the loop shared by the bounded caller drain
  and the unbounded reactor drain) now takes the per-ws lock **around each `commit_batch`**, making a
  drain's SELECT+commit atomic w.r.t. other drains. Per-batch, not per-pass, so the reactor never
  blocks an inline caller for more than one batch — the drain-backpressure guarantee is preserved.

**Docs.**

- `docs/debugging/ingest/2026-07-27-ingest-write-conflict-storm.md` (new).
- `docs/scope/ingest/drain-backpressure-scope.md` — addendum updating the "two drainers racing is
  safe" contract to reflect the retry + per-ws serialization.

## Tests

- `crates/ingest/tests/ingest_conflict_storm_test.rs` **(new, 2 tests, real `Store::memory()` = real
  SurrealDB `kv-mem`, no fakes; multi-thread runtime)**: `serialized_drains_are_exactly_once`
  (drain-vs-drain, exactly-once) and `serialized_drains_vs_gc_lose_no_samples` (drain-vs-GC, no
  sample lost). Stable across repeated runs.
- Full suites green after the change: `lb-store` (all), `lb-ingest` (all), and the host ingest tests
  `ingest_drain_bound_test` / `ingest_isolation_test` / `ingest_test`.

## Key findings (detail in the debugging doc)

- The retry primitive already existed (flows run-store rev race) but was never on the transactional
  ingest path; consolidating + one `query_ws_retrying` wrapper closed the gap with no new mechanism.
- WS-A retry alone does NOT absorb N-way *unserialized* drain-vs-drain (the 16-retry bound can
  exhaust) — which is exactly why WS-B exists. They are complementary; do both.
- Serialize at the granularity of the conflict (one `commit_batch` / one staging head), not the whole
  pass, or you re-introduce the backlog-latency coupling the drain-backpressure fix removed.

## Constraints honoured

- No git touched (no branch/commit/tag/push) — the user handles git across the parallel agents.
- No `if ext == "modbus"` (or any ext-id) branch anywhere — the fixes are generic concurrency
  hardening (Rule 10).
- No change to the 5-min/15-min retention default or `is_unmodified_default`'s recognition list.
- No repo-root workspace; no path/git dep introduced.

## NOTE for the user — lb tag bump required

rubix-ai pins `lb-node = { git = "…/lb", tag = "node-v0.11.0" }`. This fix reaches a running node
only via a **new lb tag** (e.g. `node-v0.12.0`) plus a bump in `rubix-ai/Cargo.toml`. Cutting the tag
and bumping the pin are the user's git/release steps — not performed here. WS-D (rubix-ai) can build
against this checkout in the meantime via `[patch."https://github.com/NubeDev/lb"]`.
