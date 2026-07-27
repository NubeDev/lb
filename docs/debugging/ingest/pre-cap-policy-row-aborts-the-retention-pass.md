# A retention-policy row written before `max_samples` existed aborts the whole GC pass

- Date: 2026-07-27
- Area: ingest / retention
- Status: **fixed**
- Found by: the new prior-state test (issue [#108](https://github.com/NubeDev/lb/issues/108) track A,
  mandatory testing category 6) — never by the suite, and never on a fresh install.

## Symptom

On a node upgraded from a build that predates the `max_samples` count cap (issue #65),
`series.retention.list` and **every retention GC pass** fail with:

```
Extension("value did not deserialize: Serialization error: failed to deserialize; \
 expected a 64-bit unsigned integer, found None")
```

The blast radius is the workspace, not the row: `run_gc` opens with `list_policies`, so ONE
pre-cap policy row makes the retention reactor's pass error out for that workspace on every tick —
nothing is rolled up, nothing is evicted, and the only signal is a `retention gc pass failed` log
line. Time horizons an operator configured years ago stop being enforced silently.

## Root cause

`list_policies` projects every policy column **by name** (deliberately — the closed-struct trap: a
field added to `Policy` but not to the `SELECT` reads back as its serde default forever). Naming a
column an older row never wrote returns `NONE`, i.e. a **present null**, not an absent key. So:

- `#[serde(default)]` — which is what `max_samples` and `tiers` carried — covers an **absent** key
  and never fires here;
- `u64` then refuses the null, and the whole `SELECT` decode fails.

Absent-key and present-null are two different upgrade bugs, and the `default` attribute only
addresses one of them. Every test in the suite wrote its policy rows with today's `Policy`, so every
row on test disc always carried the column.

## Fix

`crates/ingest/src/retention.rs` — a small `none_as_default` deserializer applied to `max_samples`
and `tiers`, so a `NONE` column reads as the type's default (unbounded / no tiers), which is exactly
what the row meant when the older build wrote it. No SQL-dialect change, no struct-shape change, and
serialization is untouched.

## Regression test

`crates/host/tests/series_prior_state_test.rs::a_policy_row_written_before_the_cap_existed_reads_back_unbounded_and_upgrades_in_place`
— seeds the pre-cap row shape through `tests/support/prior_state.rs` (a real `UPSERT`, byte-for-byte
the statement `set_policy` issues), then asserts the row lists, GCs as unbounded, and upgrades in
place when the new build re-sets the prefix.

Fail-before / pass-after, exact output:

```
test a_policy_row_written_before_the_cap_existed_reads_back_unbounded_and_upgrades_in_place ... FAILED
list: Extension("value did not deserialize: Serialization error: failed to deserialize; expected a 64-bit unsigned integer, found None")
test result: FAILED. 1 passed; 1 failed
```
```
test a_policy_row_written_before_the_cap_existed_reads_back_unbounded_and_upgrades_in_place ... ok
test the_new_global_cap_governs_only_after_the_previous_builds_per_network_row_is_removed ... ok
test result: ok. 2 passed; 0 failed
```

## Lesson

An explicit column projection turns "a field the old rows never wrote" from a serde default into a
hard decode error — and it lands in the FIRST call of the GC pass, so one legacy row disables a
whole subsystem. Whenever a field is added to a stored struct, the upgrade question is not "does
`default` cover it" but "does the read project it by name, and what does the engine return for a
column that isn't there". Test it by seeding the old shape (testing-scope §2 category 6), because no
test that writes with today's struct can ever produce it.
