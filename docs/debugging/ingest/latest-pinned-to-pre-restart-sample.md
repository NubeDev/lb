# `series.latest` pinned to a pre-restart sample — a live meter read stale for hours

- **Date:** 2026-07-14
- **Area:** ingest (the series plane: `ingest.write` producer stamp + `series.latest` ordering)
- **Status:** fixed
- **Found by:** live behaviour in `ems` (a real sidecar restart), **not** by a test

## Symptom

A live meter's `series.latest` kept returning the SAME old value for hours while fresh samples were
being ingested successfully. No error, no dead-letter, no gap in the committed rows — the fresh data
was in the store and `series.range` could see it. Only `latest` was stuck, and it was stuck on the
last sample written *before the producing sidecar restarted*.

## Root cause — two bugs, one shape

`seq` is monotonic **per `(series, producer)` only**. It is a *within-stream* ordering key and carries
no meaning across producers — two producers' seqs are two unrelated scales. Two independent defects
conspired so that every restart put a live series into a permanently-stale state:

**1. Every stream of one principal was flattened onto ONE producer id.**
`host/src/ingest/write.rs` overwrote each sample's `producer` with the bare `principal.sub()`. The
intent was sound and security-critical (an un-spoofable producer, so the dedup identity
`(series, producer, seq)` can't be forged to collide with another principal's stream). But it
collapsed every stream that one extension runs into a single seq space.

**2. `latest` ordered the whole series by `seq DESC`** — i.e. across producers, comparing those
unrelated scales.

Together: a producer whose in-memory `seq` counter restarts at 0 (any restarted process) re-entered
the same seq space *below its own pre-restart high-water mark*. `latest` ordered by `seq DESC`, so
the old stream's `seq=807` beat the new stream's `seq=0,1,2…` **forever**. Fresh samples landed
underneath the mark and never surfaced.

The rest of the data plane already modelled multi-producer-per-principal correctly — `commit.rs`
treats producer-A's `seq=5` and producer-B's `seq=5` on one series as two distinct rows. Only the
`ingest.write` stamp disagreed with that model.

## Why no test caught it

Worth remembering — the same shape as the `dashboard.save` cap leak triaged the same day
(`auth-caps/schema-validation-preceded-cap-gate-leaks-400.md`): **the bug was invisible to the
assertions that existed.**

- The test helper `sample()` tied `ts: seq`, so the two axes could **never disagree** in any existing
  test. A producer restart is precisely the case where they DO disagree: `seq` goes backwards while
  the wall clock goes forwards. No test in the file could express the failure.
- A **fresh** series has no prior epoch, so no green e2e run could reproduce it. It only bites a
  series that **outlives a producer restart** — i.e. every real one, and no test one.

## Fix

**Root the producer, don't flatten it.** `ingest.write` now stamps `principal.sub()` when the caller
declares nothing, else `{principal.sub()}/{declared}` (`root_producer`). The principal prefix is
still stamped by the host and un-spoofable, so the isolation property is unchanged — a caller can
only ever carve up its **own** namespace. The declared leaf is untrusted: the separator `/` collapses
to `-`, so a declared `a/b` can't forge a deeper path or re-shape an id to mimic another principal's
namespace. An empty declaration means "no sub-namespace" — the bare principal, exactly as before
(back-compatible).

**Order `latest` by the axis the streams actually share.** `ORDER BY seq DESC` →
`ORDER BY ts DESC, seq DESC`. Within one producer, `seq DESC` still breaks ties, so a producer
batching many samples onto one `ts` keeps its exact intra-batch order.

The tradeoff, stated honestly: `ts` is the producer's clock and may skew. But **a skewed clock is a
*data* problem visible to the caller, whereas the seq ordering was a *correctness* problem invisible
to everyone.**

## Regression tests

In `ingest/tests/ingest_test.rs`, on a new `sample_at()` helper that sets `ts` **independently** of
`seq` — the thing the old helper made impossible:

- `latest_follows_wall_clock_across_a_producer_restart` — a producer's seq restarts at 0; `latest`
  must follow the wall clock, not pin to the pre-restart sample.
- the intra-batch sibling — one producer batching several samples onto one `ts`; `seq` must still
  order them.

## Lesson

**A per-stream ordering key must never be compared across streams.** `seq` was documented as
monotonic per `(series, producer)`, and the code then ordered a whole series by it — the docs were
right and the query disagreed.

Two smaller ones worth keeping:

- **A test helper that ties two axes together makes a whole bug class unexpressible.** `sample()`'s
  `ts: seq` convenience is exactly why a hours-long production staleness had no failing test.
- **"Un-spoofable" and "flat" are not the same requirement.** Rooting the caller's id under an
  authenticated prefix keeps the security property while restoring the multi-stream model the rest
  of the plane already had — the fix was to make the stamp *hierarchical*, not to weaken it.
