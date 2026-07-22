# series.latest is O(rows), series.latest_many was denied, series.read scans unbounded raw

Date: 2026-07-22. Found while auditing an ems meter page that was "still really slow with 10-20 API
calls". Three independent findings; two are lb-core bugs fixed here, one is a config gap.

## 1. `series.latest` / `series.latest_many` were O(rows-in-series), not O(1) — FIXED

**Symptom.** `series.latest` on a 10.5k-sample series took ~1s; `series.latest_many` for 2 series
took ~2.6s (it had NO `LIMIT` — it fetched ALL rows for all names and sorted them in Rust). As the
series grows the cost grows linearly.

**Root cause.** SurrealDB 2.6.5 serves `... WHERE series = $s ORDER BY ts DESC LIMIT 1` with a
`MemoryOrderedLimit`: the `(series, ts)` / `(series, seq)` index is used ONLY for the `series=`
equality (which for one series matches all 10k rows), then it loads all matched rows into memory and
sorts them to take 1. EXPLAIN confirmed: `Iterate Index` (equality only) → `Collector
MemoryOrderedLimit`. There is no reverse-index-range-limit; even `ORDER BY … ASC LIMIT 1` sorts.

**Fix.** A materialized newest-sample POINTER table `series_latest` (record id = series name), advanced
FORWARD-ONLY in the SAME commit transaction as the raw write (`commit.rs`), so `latest`/`latest_many`
are point lookups (`SELECT … FROM ONLY series_latest:[series]` / `WHERE series IN $names`).
- Not a cache: it only advances when a committed sample beats the stored `(ts, seq)`, ts-primary — so
  late/replayed/restarted-producer samples never regress it (the restart trap in `latest.rs`'s
  docstring). Deleted/renamed via `delete.rs`/`rename.rs`.
- Pre-pointer series (committed before this landed) have no pointer row: `latest` cold-paths to the
  old ordered scan ONCE and lazily backfills the pointer, so old series self-heal on first read.
- Result live: `series.latest` ~1s → **10ms**; `series.latest_many` 2.6s → **10-20ms**;
  `ems.meter.verify` (which fans latest over a meter's points) 2.6s → **20ms**.
- Files: `schema.rs` (SERIES_LATEST_TABLE), `commit.rs` (tx advance), `latest.rs` (read + backfill),
  `delete.rs`/`rename.rs` (propagate). Tests: `series_plane_test.rs::latest_pointer_*`.

## 2. `series.latest_many` returned `denied` for EVERY caller — FIXED

**Symptom.** Even an admin token holding `mcp:series.latest:call` got `denied` on `series.latest_many`
(singular `series.latest` worked on the same series). The verb was absent from `tools.catalog`.

**Root cause.** The outer MCP authorize gate (`mcp/call/authorize.rs`) builds the required cap verbatim
from the tool name → `mcp:series.latest_many:call`, which is granted in NO role bundle
(`builtin_roles.rs` grants `mcp:series.latest:call` only). The inner `authorize_ingest` correctly reuses
`series.latest` (`ingest/read.rs`), but the outer gate denies first. The series-read-perf scope's
"reuses the grant, checked once" design was never wired into the outer gate.

**Fix.** Added `series.latest_many → series.latest` to `gate_tool_for` in `host/src/tool_call.rs` (the
sanctioned cap-alias seam, same as `outbox.enqueue_held`, `grants.revoke`, etc.). One mapping, two
callers (dispatcher + tools.catalog visibility). Test:
`authz_mcp_dispatch_test.rs::series_latest_many_rides_the_series_latest_cap`.

## 3. `series.read {buckets}` scans unbounded raw — CONFIG gap, not a code bug

**Symptom.** A 24h bucket read took ~4s and BLOCKED every other call on the store (a `meter.verify`
that runs in 30ms alone took 7.9s while two bucket reads ran — the store serializes them).

**Root cause.** No retention policy was ever set, so raw samples accumulate without bound (10.9k in
24h from a fast poller). The bucket `GROUP BY` (even with the correct `(series, ts)` index range scan)
must read every raw row in the window — O(rows-in-window). Both query halves (numeric agg AND
count+ordered-last) are ~1.7s each at 10k rows; the `math::floor`/`time::millis` per-row cost on the
embedded engine dominates.

**Resolution.** Set a retention policy: `series.retention.set { prefix:"modbus.", raw_for_ms,
max_samples, tiers:[{width_ms, keep_for_ms}] }` then GC. Live: evicted 36.6k raw → 1.7k raw + 1.26k
rollup rows; the same 24h bucket read dropped **4.0s → 0.7s** (rollup merge keeps the answer exact).
This belongs in the ems PACK (seed a retention policy for the `modbus.` series prefix), OR lb could
ship a sane default policy. NOT an engine bug — the engine does exactly what it's told with unbounded
data. A follow-on lb optimization (cheaper bucket agg / a "latest bucket" fast path) is possible but
secondary to bounding the raw tier.
