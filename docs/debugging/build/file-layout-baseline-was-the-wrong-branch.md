# "the merge adds no file-layout violations" measured against the wrong baseline

- Area: build / CI (`file-layout`, `rust/scripts/check-file-size.sh`)
- Status: resolved (the measurement is corrected; the 8 splits are owed)
- First seen: 2026-09-10
- Regression test: n/a — the check itself is the test. What failed was a human
  comparison, not the tooling.

## Symptom

Nothing broke. The `file-layout` CI job was red on master before #200 and red after
it, so the job's own signal never changed — which is exactly why the wrong claim
survived: a red-to-red transition looks like "no change" unless you count.

The written record of #200 (the merge of `feat/agent-invoke-mcp-arm` with master's
SurrealDB 3 upgrade and the ingest staging removal) said:

> `file-layout` stays red with the **identical 22 violations #198 already has**
> (master has 15), so the merge adds none.

## What is actually true

Measured by running `bash rust/scripts/check-file-size.sh` on both refs in
throwaway worktrees:

| ref | violations |
|---|---|
| `d109b016` (master immediately before the merge) | **14** |
| `ac8701b4` (master after #200) | **22** |

The merge adds **8**, not none:

```
rust/crates/host/src/pack/apply.rs           664 → 673
rust/crates/host/src/viz/query.rs            482 → 486
rust/crates/packs/src/manifest.rs            749 → 801
rust/crates/packs/src/validate.rs            518 → 521
rust/crates/host/tests/widget_catalog_test.rs 416 → 513
rust/crates/host/tests/viz_query_test.rs      738 → 751
rust/crates/host/tests/flows_triggers_test.rs 818 → 893
rust/crates/rules/src/grid.rs                 426   ← genuinely NEW
```

Seven are growth past an existing baseline on files already far over the 400-line
limit. One — `rules/src/grid.rs` — crosses 400 for the first time.

## Why the claim was wrong

**The baseline was the merge's own source branch, not its destination.** #198's
branch already carried those 8, so comparing merged-master against #198 shows no
delta — of course it doesn't; the branch is where they came from. The question the
`file-layout` job asks is what lands on *master*, so the only honest baseline is the
master commit the merge was made onto (`d109b016`).

The parenthetical "(master has 15)" was the right number to reason from and was
written down and then not used. 15 vs 22 is the whole finding, sitting in the same
sentence as the conclusion it contradicts.

## The lesson

A check that is red before and red after reports **no transition**, so its own
output cannot tell you whether you made it worse. For any always-red check, the
count is the signal, not the colour — and the count must be taken on the branch the
change lands on, in the same checkout, both sides.

This is the same shape as the `agent_routed` false alarm recorded earlier in the
same session: in both cases the fix was to re-measure against the correct parent
rather than trust a plausible story about where a failure came from.

## Follow-up owed

The 8 are not a reason to hold a release — every one is inside the pre-existing
backlog's shape — but they must not be recorded as zero. `rules/src/grid.rs` is the
one worth splitting first, since it is a new violation rather than more of an old
one.
