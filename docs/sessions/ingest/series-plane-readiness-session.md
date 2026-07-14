# Session — series-plane readiness (issues #55–#58, one PR)

Date: 2026-07-14 · Branch: `series-plane-readiness` · Closes #55 #56 #57 #58

Four slices that take the `series` plane from "works in a demo" to "won't fall over": schema +
time semantics, keyset paging, bucketed decimation, retention/rollup/GC. All additive on the
existing `series.read` cap except the new admin-tier `series.retention.*` verbs.

## What shipped

### Slice A — schema + time semantics (#55, `lb_ingest::schema/meta/labels`)

- `ensure_series_schema` (called by the commit worker; process-local once-per-ws guard): migrates
  legacy numeric `ts` → `datetime` (type-guarded, idempotent), `DEFINE FIELD ts TYPE datetime`,
  `DEFINE INDEX` on `(series, seq)`, `(series, ts)`, and the rollup table's `(series, width_ms, t)`.
- Commit stores `ts` as a real datetime (`time::from::millis`; wire `ts` is epoch ms); reads project
  it back via `time::millis` so the `Sample` wire shape is unchanged.
- **Label→tag commit:** `Sample.labels` now convert to real tag-graph edges on `series:<name>`
  (`Source::Producer`), once per series (`series_meta.labels_applied` latch) — `series.find` finally
  finds what ingest wrote.
- **Cardinality cap:** `series_meta` registry (one row per distinct series name);
  `commit_batch` admits a NEW name only under the cap (default 10 000, `commit_batch_capped` for
  explicit caps); over-cap samples are **dead-lettered** (reason `series-cap`) in the same tx —
  never silently dropped, never a new index entry. `CommitPass` gained `dead_lettered`.
- `series.read` accepts wall-clock `{from, to}` (epoch ms, half-open) alongside `from_seq`/`to_seq`
  — the wire shape ems' `fetch-history.ts` already sends.

### Slice B — keyset paging (#56, `lb_ingest::{cursor,page}`)

`series.read {limit, cursor, direction}` → `{samples, next_cursor, prev_cursor}` (the `samples` key
is kept from the pre-paging shape). Seek key is the **unique composite `(seq, producer)`** —
producer breaks the seq tie, so a chain never skips/repeats. Opaque versioned cursor
(`base64("v1:<seq>:<producer>")`); it carries no workspace/series (a bookmark, never a grant — every
page re-authorizes `mcp:series.read:call`). Default AND max `limit` = 10 000; unpaged callers now get
a bounded first page instead of an unbounded `Vec` (the intended behavior change). Internal raw
`read()` kept for in-process callers (viz, flows).

### Slice C — bucketed decimation (#57, `lb_ingest::bucket`)

`series.read {mode:"buckets", from, to, width_ms|budget}` → `{buckets: [{t, min, max, avg, last,
count}], width_ms}`, ≤ budget (hard cap 2 000 buckets). **Deviation from the scope's SurrealDB
`GROUP BY time_bucket`:** SurrealDB 2 has no ordered `last` aggregate, so decimation is a **chunked
fold over the keyset pager** (10 k rows/chunk — O(page) memory, rides the `(series, ts)` index):
`last` is exact (max `(ts, seq)`), non-numeric payloads tolerated (count + last, no min/max/avg).
Spike-survival proven by test (a 200° spike inside a bucket shows in `max`, avg ~flat).

### Slice D — retention + rollup tiers + GC (#58, `lb_ingest::{retention,rollup,gc}`)

New scope doc: [`../../scope/ingest/series-retention-scope.md`](../../scope/ingest/series-retention-scope.md).
Per-prefix policy `{prefix, raw_for_ms, tiers:[{width_ms, keep_for_ms}]}`; verbs
`series.retention.set/list/delete/gc` (admin-tier caps in `builtin_roles`; `delete` rides the `set`
cap). GC = rollup-then-evict, cutoff snapped to the widest tier boundary (only complete buckets roll
up); rollup rows carry `sum`/`num_count` for exact re-aggregation; bucketed reads merge the finest
tier where raw was evicted. `now_ms` is caller-injectable (determinism §3).

### Host wiring

`host/src/ingest/{read,retention,tool}.rs` (+ mod), `system/catalog.rs` entries,
`authz/builtin_roles.rs` admin caps.

## Tests (green)

- `cargo test -p lb-ingest` — 15 tests: prior 8 + `series_plane_test.rs` (paging exactly-once walk
  incl. producer-tie, back direction, malformed-cursor reject, cursor round-trip, wall-clock window,
  bucket budget + spike survival + last/avg, cardinality cap dead-letter + existing-series pass,
  label→tag via `lb_tags::find`, GC rollup/evict/tier-evict/idempotent + rollup-backed buckets).
- `cargo test -p lb-host --test series_plane_host_test` — 4 tests: MCP paged chain, buckets via MCP,
  **mandatory capability-deny** (both read modes + all four retention verbs), **mandatory workspace
  isolation** (ws-B replaying a ws-A cursor sees nothing; ws-B gc/list touch nothing of ws-A).
- Full `cargo test --workspace` green (see PR).

## Debugging

One test-authoring gotcha logged:
[`../../debugging/ingest/equal-seq-drain-order-nondeterministic.md`](../../debugging/ingest/equal-seq-drain-order-nondeterministic.md).

## Deliberate deviations / notes

- The paging scope's slice A ("page-cursor" shared codec crate) is unbuilt; the cursor codec shipped
  inside `lb_ingest::cursor` and can be lifted into the shared primitive when slice A lands.
- The issue's 5M-sample perf gate + `.explain()` plan assertion are NOT in CI (too heavy for the
  default suite); the indexes are defined and named, and the fold is chunk-bounded. Follow-up if a
  perf harness lands.
- LTTB, dense-vs-sparse buckets (sparse shipped), budget-vs-width primary (both shipped, width wins)
  recorded as resolved-in-implementation in the decimation scope's open questions.
