# Retention GC silently dies on an integer `sum` — the store then grows forever

**Date:** 2026-08-03
**Box:** RC-6 (Rubix Compute, 959 MB, armv7)
**Fix:** `rust/crates/ingest/src/rollup.rs` (+ the commit-path twin in `filter.rs`)
**Related:** NubeDev/lb#128 (store boot OOM)

## Symptom

With 30 modbus meters (1680 points) polling into a fresh store, the store grew
**linearly at ~3.5 MB/min for 30 minutes and never plateaued** — straight through the
15-minute raw window where the 5-minute `avg` fold should have flattened the curve.

```
  min   storeMB   deltaMB
    0      22.8     +0.00
    5      80.9     +3.50
   15     115.9     +3.51      <- raw window elapsed; fold should start here
   30     169.0     +3.48      <- still perfectly linear
```

`series.retention.status` showed the reactor had run exactly ONCE and then frozen:

```json
{"last_run_ms": 1785729931112, "duration_ms": 30416,
 "evicted_raw": 0, "rollup_rows": 0, "warnings": []}
```

`last_run_ms` never advanced again despite `RETENTION_PERIOD` being 300 s. Nothing in
the journal. `warnings: []`.

Calling the verb by hand gave the only visible clue:

```
series.retention.gc -> extension error: value did not deserialize:
  Serialization error: failed to deserialize; expected a 64-bit floating point,
  found 6432914451i64
```

## Cause

`RollupRow.sum` is `f64`, written with `json!(r)` and read back with `resp.take(0)`.
**SurrealDB stores a whole-numbered float as an `i64`.** A meter reading that happens to
land on an exact integer therefore round-trips as an integer, and the derived
`Deserialize` rejects it — failing the whole row, and with it the whole GC pass.

The failure is **silent and self-perpetuating**:

1. Pass 1 finds no rollup rows yet (nothing to read), folds raw into buckets, writes them.
2. Pass 2 calls `read_rollups` over what pass 1 wrote, hits an integer `sum`, and dies.
3. Every later pass dies the same way. Retention never evicts again.

So the feature whose entire job is bounding growth stops, quietly, the moment it has
done its job once. This is a plausible contributor to the 617 MB store behind #128 —
that box also had no `LB_STORE_MAX_BYTES`, so nothing else was bounding it either.

## Fix

`de_lenient_f64` / `de_opt_lenient_f64` — accept either numeric shape on the way in.
The writer cannot control which one the datastore picks, so the reader must tolerate
both. A non-numeric value is still a hard error; the leniency is about numeric SHAPE,
not anything-goes.

### There are FOUR decode sites, not one

Fixing `RollupRow` alone was NOT enough — the deployed binary still failed, with a
different value (`20094673954i64`). Patch the whole class or the bug just moves:

| site | why it narrows | blast radius |
|---|---|---|
| `rollup::RollupRow.{sum,min,max}` | a whole-numbered sum persists as `i64` | GC pass 2+ |
| `bucket::NumRow.{min,max,sum}` | **SurrealDB's own `GROUP BY` aggregate** returns an integer for `math::sum` over integer samples | every bucketed read, and the GC fold that drives it — this is the SOURCE the rollup row is written FROM |
| `filter::LastCommitted.value` | an integer meter reading persists as `i64` | the COMMIT path (breaks ingest, not just GC) |
| `filter::{Range,Deadband}` | an operator writes `{"max": 100}` — integers are the natural way to author a bound | any policy read carrying a filter |

The audit that found the last three (run it after adding any persisted `f64`):

```sh
# every Deserialize struct carrying an f64, flagged if it lacks the lenient attr
python3 - <<'PY'
import re,glob
for f in glob.glob('crates/ingest/src/**/*.rs',recursive=True):
    src=open(f).read()
    for m in re.finditer(r'((?:#\[derive[^\]]*\][\s\S]{0,200}?)?struct\s+(\w+)\s*\{[\s\S]*?\n\})',src):
        blk,name=m.group(1),m.group(2)
        if 'f64' in blk and 'Deserialize' in src[max(0,m.start()-300):m.start()+120]:
            ok='lenient_f64' in blk
            print(('OK  ' if ok else 'RISK'),f+':',name)
PY
```

`bucket::NumRow`'s existing comment on its `b: i64` field already understood this exact
hazard — "`u64` would fail the DECODE rather than clamp, taking the read down instead of
returning a short first bucket" — but the numeric columns beside it were left strict.

### The same bug on the commit path

`filter::LastCommitted.value: Option<f64>` is persisted to `series_meta` and read back
in `filter_state::read_filter_state`. Identical narrowing, but **worse**: it is on the
commit path, so an integer-valued meter behind a deadband/min-interval filter would
break ingest itself, not just GC. Fixed the same way.

It did not fire in this repro only because no filter was configured — `needs_state()`
is false for an inert filter, so the anchor is never read.

## Regression tests

`rollup.rs`: `integer_sum_deserializes`, `float_sum_still_deserializes`,
`null_min_max_is_none`, `non_numeric_sum_still_errors`.
`filter.rs`: `integer_anchor_value_deserializes`,
`float_and_null_anchor_values_still_work`.

`cargo test -p lb-ingest` green (23 lib + all integration), clippy + fmt clean.

## The other half of what this repro found — the ingest write path

`ingest::write` loops **per sample**, issuing ~2 SurrealDB queries each
(`enforce_bound` + `append_one`). One 1680-sample batch = ~3360 sequential round-trips.
`overflow::staged_count` additionally runs `SELECT count() FROM ingest_staging GROUP ALL`
— a full aggregate scan — once per sample, so cost climbs as staging fills.

Measured on RC-6: `lastPushMs` 41 s cold / ~14 s warm for one batch, at ~205% CPU.
At `pollMs: 1000` every push timed out and `lastAccepted` stayed 0 — **no data landed at
all**. 1680 points needs `pollMs >= 60000` to be even marginally stable.

The modbus extension batches correctly (one `ingest.write` per tick per network); the
host then un-batches it.

### Fixed here: the headroom short-circuit

`write` now takes ONE `staged_count` up front and skips `enforce_bound` entirely while it
holds proven headroom, falling back to the real per-sample enforcement once headroom runs
out. This halves the query count and removes the quadratic full-table scan, without
changing what the bound means — the fallback still re-counts authoritatively.

`headroom` is only ever decremented, never trusted upward. Concurrent writers can consume
real headroom underneath it, which is safe for the same reason the original code was: the
bound is a coarse backpressure cap (see `overflow`'s module doc), not an exact quota.

Regression tests in `ingest_test.rs`: `one_batch_larger_than_the_bound_still_stays_bounded`
(the case the pre-existing overflow tests could NOT see — they write one sample per `write`
call, so every check was a fresh call) and `a_batch_within_headroom_stages_every_sample`.
Both were confirmed load-bearing by removing the fallback and watching the first one fail.

### Still NOT fixed — batching the staging UPSERTs

`append_one` is still one query per sample. Collapsing a batch into a single multi-row
UPSERT is the remaining (larger) win, but it is a real change to the write path's shape
rather than a bug fix — `overflow`'s module doc already flags the checkpointed-ring
optimization as deliberately out of scope for that slice. Left for an owner decision.

---

## Verified on RC-6 (2026-08-03)

The fix is now **proven on the box**, not just in tests. Node at `192.168.15.10:8099`
(the RC holds two addresses on `eth0` — `.15.11` primary and `.15.10` secondary — which
is why the IP appears to drift between sessions).

Two traps had to be cleared before the test could mean anything:

1. **The service was not running.** After a power cycle the unit was `inactive (dead)` and
   `disabled`, so `:8099` refused connections. Starting it triggered a store-segment
   repair (`Corrupted transaction record ... segment_id: 2`) — expected recovery from the
   interrupted write, not a new fault. Boot completed clean.
2. **The clock was 4 hours slow** (no RTC). GC evicts on *age*, so a slow clock makes
   every aged sample look fresh and pins `evicted_raw` at 0 for reasons unrelated to this
   bug. Always `sudo date -u -s ...` before judging a GC result.

The `modbus.loadtest.*` series preserved for this proof were **empty husks** — the boot
compaction had already consumed their samples, so they could not demonstrate eviction.
Fresh probes with controlled timestamps were used instead, which is the stronger test:
integer payloads (the exact shape that triggered the narrowing bug), straddling the
15-minute raw window so that correct behaviour must *keep* one group and *evict* the other.

| step | result |
|---|---|
| `series.retention.gc` returns JSON, not `expected a 64-bit floating point, found <N>i64` | ✅ |
| `last_run_ms` advances (was frozen for 80+ min) | ✅ |
| 60 samples aged 40 min → `evicted_raw: 60`, `rollup_rows: 3` | ✅ |
| 30 samples aged 2 min → all 30 survive, every one newer than the window boundary | ✅ |
| GC pass 2 re-reads pass 1's rollup rows (`RollupRow` site) | ✅ clean, idempotent |
| **reactor fires unattended on its 300 s timer** → `evicted_raw: 40`, `rollup_rows: 2` | ✅ |
| warnings across every pass | 0 |

The last row is the one that matters: eviction and the 5-minute fold now happen with no
manual call at all. That is the loop that had been silently dead.

Note `strings | grep de_lenient_f64` is **not** a usable deployment check — the serde
helper is inlined and its name does not survive into the stripped armv7 binary. It reads
as absent in both the fixed and pre-fix binaries. Verify by running the GC call and
reading the error, or compare checksums against a known-good build.

Probe series were deleted after the run; store settled at 28 MB.

### A working GC is necessary but not sufficient

This bug made the GC *dead*. Fixing it does not make a node's disc **bounded** — the stock
modbus policy keeps its rollup tier forever (`keep_for_ms: 0`), so a healthy GC still folds
raw into rollups that accumulate without limit. At 1800 points that is ~193 MB/day.

The operator-side sizing for that — measured constants, the horizons that actually fit an
RC's disc, and the two other guards found inert on the same box — is in
`rubix-fleet/docs/rasp-pi/CAPACITY-AND-LIMITS.md`.
