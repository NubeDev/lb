# `delete_series` leaves the series' dead letters behind

**Area:** ingest / series lifecycle · **Date:** 2026-08-07 · **Status:** fixed
**Fix:** `crates/ingest/src/delete.rs` · **Regression test:**
`crates/ingest/tests/series_dead_letter_gc_test.rs::deleting_a_series_removes_its_dead_letters_and_no_others`

## Symptom

`series.delete` reports success and the series is gone from every read path —
`series.read`, `series.latest`, `series.list`, `series.find` all agree it does
not exist. But its rows in `ingest_dead_letter` survive, for up to the
dead-letter horizon (30 days).

Nothing surfaces them, so nothing looks wrong. The cost is disk that a delete
was supposed to reclaim, plus diagnostic rows about a series that no longer
exists.

## How it was found

From the consumer side, not from lb. The out-of-tree `modbus` extension gained
delete-cleanup: deleting a network/device/point now purges the series that
config produced, via `series.delete`. Walking the footprint that verb clears
against the tables `crates/ingest` actually writes showed one gap —
`delete_series` cleared `series`, `series_rollup`, `series_latest`,
`ingest_staging`, `series_meta` and the tag edges, but never
`ingest_dead_letter`, even though `overflow.rs` and `commit.rs` both write there
under the same series name.

Invisible to the existing suite because every `delete_series` test asserts on
the READ paths, and no read path touches the dead-letter table.

## Why it was written that way — and why that reasoning does not extend here

`prune_dead_letters` (disk-budget scope, decision 7) deliberately keeps dead
letters on their **own** 30-day horizon, longer than the data that produced
them, so that tightening a series' `raw_for_ms` to debug a disk problem cannot
destroy the evidence of *why* rows were diverted. That is a real and correct
argument — about a **retention pass**: automatic, prefix-scoped, and not aimed
at any particular series.

An explicit `series.delete` is the opposite case. An operator names one series
and destroys it. There is nothing left for the evidence to be evidence *of*, and
"I deleted this to reclaim the disk" must not leave a month of its rows behind.

The distinction is now stated in both module headers so it is not re-litigated
in either direction: the GC keeps them, the explicit delete takes them.

## The fix

One statement in the existing multi-statement query — same transaction, same
bound `$series`, same idempotence (an unknown series matches nothing):

```
DELETE ingest_dead_letter WHERE sample.series = $series;
```

`sample.series`, not `series`: a dead letter nests the whole sample under
`sample`, which is also why the staging delete beside it uses the same path.

## What the test pins

Two series dead-lettered through the **real** overflow path (`write` with a
staging bound of 1 + `MustDeliver`), never hand-inserted rows — so the thing
under test is what the shipped divert actually writes. Delete one; assert the
other's rows survive. The control matters: the failure mode of an over-eager fix
is a table sweep that takes every producer's diagnostics with it.

## Blast radius

None on callers: `delete_series`'s signature and contract are unchanged, and it
was already documented as clearing the series' whole footprint — this makes that
true. `rename_series` is deliberately **not** changed: a rename carries a live
series to a new name, and its dead letters are diagnostics about the old name's
divert events. Whether they should follow the rename is a separate question this
fix does not answer.
