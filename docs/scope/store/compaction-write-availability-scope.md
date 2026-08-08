# Store scope — compaction write availability: shrink (then remove) the stop-the-world pause

Status: scope (the ask). Driven by a live incident on a rubix-ai edge node (RC-6, armv7,
959 MB RAM, 2026-08-08): every hourly budget-driven `store.compact` pass held the store's
write guard for **~94 s**, during which every `ingest.write` timed out and the modbus
extension dropped ~90 s of meter data per network — despite `qos: "must-deliver"`.
Downstream trackers: `NubeIO/rubix-ai` (RAM-aware budget) and
`NubeIO/rubix-ai-extensions` (modbus re-push queue) — this scope owns the upstream half.

## The problem, measured

- The online pass (`rust/crates/store/src/compact.rs` `compact()`) takes the handle's
  WRITE guard for the whole pass: quiesce-wait (2–30 s) → whole-log blocking file I/O →
  reopen → swap. Every store op node-wide waits. On a 543 MB log the pass measured
  93.7 s; journal shows `ingest.write` timeouts aligned exactly with the pass window.
- The pass is not rare on an ingesting node, because the log regrows fast: **every
  committed sample costs ≥3 log appends** on the append-only engine — the staging UPSERT
  (`crates/ingest/src/write.rs` `append_one`), the series UPSERT at commit
  (`crates/ingest/src/commit.rs`), and the staging DELETE tombstone in the same commit
  tx. Measured on RC-6: 36 MB → ~540 MB/hour at a modest two-network modbus poll.
- Engine memory tracks the log (~1.4× log bytes for key-dense ~700-byte samples —
  `crates/store/src/boot_guard.rs` `BOOT_COMPACT_MEM_RATIO` doc), so the log high-water
  mark is also the node's RSS high-water mark. Write amplification is therefore a
  **memory** problem as much as a disk one.

## Goals

1. **Bound the write-unavailability window**, independent of log size. Target: no single
   pass blocks writers longer than a producer's push budget (≤5 s on the shipped modbus
   default).
2. **Reduce ingest's per-sample log amplification** (≥3 appends → as close to 1 as the
   exactly-once staging contract allows).
3. Keep the merge-completion rule (P0 in `compact.rs`) intact — no writing session may
   ever apply a pending `.merge/`.

## Non-goals

- Replacing SurrealKV (persistent-backend-scope owns engine selection).
- Client-side buffering during the pause (that is the extensions' half; a shorter pause
  is still required because not every producer will buffer).
- Changing the budget marks / trigger policy (disk-budget-scope owns when a pass runs;
  this scope owns what a pass costs).

## Intent / approach

Two independent levers, either of which relieves the incident; do (1) first because it is
contained in this crate.

1. **Amplification: let inline commits skip staging.** The staging round-trip exists so a
   burst hits a cheap unindexed path and so an offline producer's re-append is idempotent.
   But the dominant edge path is a live producer whose caller-path drain
   (`drain_workspace_bounded`) commits its own samples within the same request. For that
   path, staging-append + immediate-drain + tombstone is pure amplification: commit the
   batch directly to `series` (same `[series, producer, seq]` upsert, same tx) and write
   staging rows only when the direct commit cannot run. Alternative rejected: batching
   multiple samples per staging row — it breaks the per-sample composite-id dedup the
   whole exactly-once story is keyed on.
2. **Pause: compact against a snapshot, swap at the end.** Today the pass quiesces first
   and holds everything for the whole file rewrite. Invert it: leave the live engine
   serving, run `surrealkv::compact()` on a copied/hard-linked snapshot of the log dir,
   then take the write guard only for the final catch-up replay (appends since the
   snapshot) + swap — the guard hold becomes O(delta), not O(log). Risks below; if the
   engine cannot support it, the fallback is segment-incremental compaction (bound each
   pass to N segments per `max_segment_size`), which bounds the pause without inverting
   the design.

## How it fits

- One datastore, symmetric nodes: both levers live below the host seam; no role branch.
- The exactly-once contract (`(series, producer, seq)` upsert) is preserved by
  construction in lever 1 — the direct commit uses the identical key.
- Capability surface unchanged: `store.compact` stays a job; `ingest.write` semantics
  unchanged (callers just stop paying 3× bytes).
- Rule 9 (no mocks): soak evidence comes from a real armv7 node; the RC-6 journal is the
  baseline to beat.

## Example flow (lever 1)

1. modbus pushes 40 samples → `ingest.write`.
2. Host authorizes, stamps the producer, and calls the direct commit: ONE tx upserting 40
   `series` rows + rollup/filter maintenance. No staging rows, no tombstones.
3. The drain reactor finds staging empty (only crash-recovery or bursty backlogs land
   there) — its tick is a no-op.
4. Log growth per sample drops ~3× → soft-mark crossings (and passes, and RSS high-water)
   stretch ~3× apart.

## Testing plan

- Unit: direct-commit path preserves exactly-once on re-push of an identical batch
  (existing `ingest_conflict_storm_test` shape).
- Integration: writer-latency histogram while a pass runs — assert p99 write stall under
  the target with a seeded ≥500 MB log (real store, real files, no mocks).
- Soak (platform-targets): armv7 node at the RC-6 workload; assert no `ingest.write`
  timeout across ≥3 compaction cycles and RSS high-water reduced accordingly.
- Regression: the merge-completion P0 test suite unchanged and green.

## Risks & hard problems

- Snapshot-compaction (lever 2) needs the engine to tolerate compacting a dir the live
  process still appends to via its copy — if surrealkv's manifest forbids it, the
  catch-up-replay design collapses to the segment-incremental fallback.
- Lever 1 moves indexed-write cost onto the caller's path; the drain-backpressure scope's
  bound (O(batch), never O(backlog)) must still hold — direct commit is per-request batch
  only.
- A crash mid-direct-commit must be provably atomic (single tx — same guarantee the drain
  relies on today).

## Open questions

1. Can `surrealkv::Store::compact()` run against a hard-linked snapshot at the pinned
   version, or does the manifest lock the dir identity?
2. Does lever 1 leave any caller that NEEDS staging's cheap-append relief (webhook
   bursts?) — if so, gate direct-commit on batch size.

## Related

- `docs/scope/store/online-compaction-scope.md` (the pass this scope bounds)
- `docs/scope/store/disk-budget-scope.md` (when a pass triggers)
- `docs/scope/store/boot-memory-guard-scope.md` (the 1.4× memory ratio evidence)
- `docs/scope/ingest/drain-backpressure-scope.md` (the caller-path drain lever 1 rides)
- Downstream: `NubeIO/rubix-ai` docs/scope/ingest/ram-aware-budget-scope.md;
  `NubeIO/rubix-ai-extensions` docs/scope/modbus/modbus-push-retry-scope.md
