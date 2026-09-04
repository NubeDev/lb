# Store scope — compaction write availability: shrink (then remove) the stop-the-world pause

> **Lever 1 went further afterwards.** This scope halved the write amplification by taking a direct
> commit *whenever staging was empty*, keeping the staged path for the cases staging was believed to
> serve. Those cases were then measured and the belief did not survive: **staging is removed
> entirely** and the direct commit is the only path
> ([`../ingest/remove-staging-scope.md`](../ingest/remove-staging-scope.md)). Three writes per sample
> become one unconditionally, not conditionally.

Status: **BUILT (lever 1) / lever 2 REJECTED at the pinned engine** — 2026-08-09, NubeDev/lb#152.
See "What actually landed" at the bottom: the direct-commit path shipped, and snapshot-compaction
was disqualified by reading surrealkv 0.9.3 rather than left as an open question.
Originally: scope (the ask). Driven by a live incident on a rubix-ai edge node (RC-6, armv7,
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


---

## What actually landed (2026-08-09)

### Lever 1 — direct commit: **BUILT**

`crates/ingest/src/direct.rs` (`commit_direct`) commits a caller's live batch straight to the
series plane, and `crates/host/src/ingest/write.rs` (`take_path`) chooses it **iff staging is
empty**. Three record writes per sample become one: no staging row, therefore no tombstone.

Two things came out differently from the scope's sketch and are worth carrying forward:

- **The decision lives in the host, not in `lb_ingest::write`.** The first attempt put it inside
  `write`, which broke the durable-redrain crash tests — correctly, because they exist to prove the
  staged recovery path and `write` is the primitive that path is made of. `lb_ingest` now exposes two
  primitives (`write` = stage, `commit_direct` = commit now) and the ONE caller-path seam that pairs
  write-with-drain picks between them, which covers the MCP verb, the gateway route, the webhook and
  the federation mirror without four call sites drifting.
- **The direct commit must chunk.** `commit_staged` builds one statement per sample into a single
  transaction, so an unchunked direct path would put an unbounded transaction on a request path.
  `DIRECT_COMMIT_BATCH` (256) matches the drain's `COMMIT_BATCH` for the same stated reason.

Open question 2 is answered: no caller needs staging's cheap-append relief on the direct path,
because the gate is emptiness — every burst, offline re-append and crash recovery has a non-empty
staging by definition and is therefore untouched.

### Lever 2 — snapshot / segment-incremental compaction: **NOT FEASIBLE at surrealkv 0.9.3**

Open question 1 was the load-bearing one and the answer is no, on three independent grounds read out
of the pinned crate source:

1. The active segment is appended **in place** (`log/mod.rs` `OpenOptions::append`, `aol.rs` seal-and-
   roll), so a hard-linked snapshot's newest segment keeps growing under the compactor. Forcing a
   rotate first is only reachable through `compact()` itself.
2. **There is no delta-replay path.** Nothing public merges "segments written since the snapshot"
   into a compacted store, and segment headers validate the segment id, so hand-renumbering a
   post-snapshot segment into the compacted `clog/` fails validation. That is a fork, not an
   integration.
3. Segment-incremental is not a mode that exists: the `CompactedUpToSegment` watermark is hardcoded
   to "everything before the rotate" and the algorithm walks the whole in-memory index, not segments.

One genuinely useful finding did come out of it: the manifest does **not** pin the directory
identity, so a copy of the store dir at another path opens, compacts, and can be moved back. That
keeps the copy-then-swap design alive *if* lb ever grows an application-level journal to replay the
writes made during the window — which is an lb design question, not a surrealkv one.

### What shipped for the pause instead

- **`CompactionPhases`** on every `CompactionRecord` (`quiesce_ms` / `open_ms` / `compact_ms` /
  `merge_ms`), returned by the `store.compact` job and persisted beside the store. The 94 s is split
  between a full sequential replay of the uncompacted log and a scattered pread-per-value pass, and
  which one dominates is a property of how much of a given node's log is live — a thing to measure on
  the node in front of you, not to reason about. This is that measurement.
- **A real value cache on the compaction handle** (`COMPACTION_VALUE_CACHE_BYTES`, 256 MiB). The
  stock `max_value_cache_size` is 100_000 **bytes** — surrealkv weighs that cache by byte — so
  compaction's per-value `resolve()` reads were running against a ~100 kB cache. Runtime-only, so
  surrealdb's own handle is unaffected.
- **Overlapped quiesce detection.** The fd-release fast path and the (size, mtime) stability
  fallback are independent proofs and both are pure reads, but they ran in series: every real node
  has the surrealdb index-builder leak, never reaches fd-zero, and therefore always waited out the
  full 5 s fast-path window *before* the 2 s stability clock could start. Sampled concurrently, the
  leaked-holder case settles at ~2 s instead of ~7 s — subtracted directly from the write-
  unavailability window, since quiesce-wait is held under the write guard.

### Still owed

Goal 1 — "no single pass blocks writers longer than a producer's push budget" — is **not met**, and
cannot be at this engine version by shrinking the pass. What remains available is keeping the log
small enough that an O(log) pass is short, which is trigger policy (`disk-budget-scope`) and this
scope's stated non-goal; the downstream `NubeIO/rubix-ai` RAM-aware budget moves exactly that number.
The route to a genuinely bounded pause is vendoring surrealkv to reorder `Core::new`
(`restore_from_compaction` before `initialize_clog`, which also retires this module's P0 dance) and
to expose `rotate()` + `compact_up_to(segment_id)`. Both are small, surgical diffs upstream.

Also owed: RC-6 soak evidence for lever 1 (log growth per hour and RSS high-water at the real
workload), and a reliable commit-log meter — the log cannot be metered in-process because surrealkv
flushes asynchronously and the index-builder leak means the handle is never fully shut down, so the
same run measured 667,665 B and 1,647,313 B minutes apart. The lever-1 tests are structural for that
reason.
