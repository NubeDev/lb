# A fully-filtered batch stops the drain loop, stranding the rest of the backlog

- Area: ingest
- Status: **fixed** (caught pre-merge, during the series-normalize slice — never shipped)
- First seen: 2026-07-26, writing the store-backed filter tests for series-normalize
- Session: ../../sessions/ingest/series-normalize-session.md
- Scope: ../../scope/ingest/series-normalize-scope.md
- Regression test: `rust/crates/host/tests/series_normalize_test.rs` →
  `a_fully_filtered_backlog_drains_completely_instead_of_stalling_after_one_batch`
- Related: [write-drains-whole-workspace-backlog.md](write-drains-whole-workspace-backlog.md) — the
  same failure class, one door over.

## Symptom

With a `filter` on a retention policy that discards most or all of a prefix's samples (the headline
case being `drop: true`, an accept-but-store-nothing mute), one `drain_workspace` call consumes **at
most one 256-row batch** and returns. Staging keeps growing; the background ingest reactor recovers
only 256 rows per 2s tick regardless of how deep the backlog is.

Measured by the regression test, against 700 muted staged rows:

| | rows drained by ONE `drain_workspace` |
|---|---|
| Broken | **0** |
| Fixed | 700 |

Zero, not 256 — the very first pass commits nothing, so the loop breaks before it has counted
anything at all. On a muted prefix, the drain becomes a complete no-op and staging grows without
bound while `committed` honestly reads 0 forever.

## Cause

`drain_at_most` (`crates/host/src/ingest/drain.rs`) used **`pass.committed == 0`** as its
"staging is empty" signal:

```rust
let pass = commit_batch(store, ws, COMMIT_BATCH).await?;
if pass.committed == 0 { break; }
```

That equivalence held for the entire life of the drain loop, because before this slice every staged
row that a batch dequeued either committed to `series` or was dead-lettered by the cardinality cap.
Series-normalize introduced a **third** outcome: a row that is dequeued and stored *nowhere*. So
"committed nothing" stopped meaning "there was nothing to do" and started also meaning "everything
in that batch was filtered" — and the loop could no longer tell the two apart.

The same latent assumption existed in the test helpers (`seed`) and would have bitten any future
caller writing the obvious loop.

This is the **drain-backpressure failure class re-entered through a new door**: not a bad line, but
a *progress signal that quietly stopped measuring progress*. Worth recording precisely because the
code looked untouched — the regression was introduced by a change in a different crate.

## Fix

Make the pass report what it **dequeued**, not just what it committed, and branch on that:

```rust
// crates/ingest/src/commit.rs
impl CommitPass {
    pub fn drained(&self) -> usize {
        self.committed + self.dead_lettered + self.filtered.dropped()
    }
}

// crates/host/src/ingest/drain.rs
if pass.drained() == 0 { break; }
```

`DrainPass` additionally carries the per-reason `filtered` counts out to the reactor, so an operator
who sets a deadband and watches `committed` collapse can see **where** the samples went rather than
inferring it.

## Why the test catches it

The regression test stages **700** rows — three batches — under a `drop: true` policy and asserts a
single `drain_workspace` accounts for all 700. A one-batch test would have passed against the broken
code, since the bug is invisible until the loop is required to iterate. Revert-checked: restoring
`pass.committed == 0` turns it red (`left: 0, right: 700`).

## Lesson

When a code path gains a new terminal outcome for a unit of work, every **loop-termination
condition** that enumerated the old outcomes is now wrong — even the ones in other crates that the
change never touched. Grep for the counters, not for the call sites.

Tracked as a class — with the two sibling bugs from the same session — in
[#108](https://github.com/NubeDev/lb/issues/108); the shapes are catalogued in
`scope/testing/testing-scope.md` §3.2.
