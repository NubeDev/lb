# viz — a panel's `timeFrom` asked for a 21.6-SECOND window instead of six hours

Status: **fixed** (2026-09-09, dashboard API-gaps review).

## Symptom

None. That is the point.

A panel setting `queryOptions.timeFrom: "6h"` would query `[now - 21_600, now]` against a clock in
epoch **milliseconds** — a 21.6-second window instead of a six-hour one, 1000× too small. The result
is a panel that renders **empty**: no error, no diagnostic, no `state:"error"` frame. Just a chart
with nothing in it, which reads exactly like "no data in this range" — the one failure mode nobody
investigates.

`timeShift` had the same defect, and applied even without a `timeFrom`.

## Root cause

`crates/host/src/viz/time_override.rs` declared — twice, in its own module header — that it worked in
epoch **seconds**:

> the host applies the override only to a target's **numeric epoch-second** `from`/`to` args (the
> `series.read` contract — the one range vocabulary the platform has)

That parenthetical is factually wrong, and it is the whole bug. `series.read` demands epoch **ms** and
says so in its own errors (`ingest/tool.rs`: `"buckets mode needs from (epoch ms)"`). So does
`viz::resolution` (it derives a bucket width from an ms-valued `to - from`), and so do the SQL macros
(`$__timeFrom` expands to ms). The client sends ms. Every consumer on the path agreed on
milliseconds; this one module asserted seconds, and its `parse_duration_secs` returned seconds that
were subtracted straight from an ms clock.

## Why it survived

**No cell had ever set one.** Measured across all 44 dashboards / 269 cells on a real bench: zero
`timeFrom`, zero `timeShift`. The code path is live and unconditional (`viz/query.rs` calls
`apply_time_override` on every target dispatch), but the feature was never used, so the arithmetic
was never exercised by anything but its own unit tests.

And those tests **agreed with the bug**. They used a seconds-scale clock (`now = 1_000_000`, seeded
`ts = 1..4`), so `"1m"` meaning 60 rather than 60_000 was self-consistent and green. The integration
test in `viz_query_test.rs::panel_time_override_applies_to_target_dispatch` did the same over a REAL
`series.read` dispatch — it asserted that a `timeShift: "1m"` over `[61, 100]` found the seeded rows,
which is true only under the broken semantics.

That is the trap worth naming: **a test written from the same wrong premise as the code will confirm
it forever.** The unit tests were not weak — they were thorough, and thoroughly wrong, because both
sides read the same mistaken header sentence.

## The fix

`parse_duration_secs` → `parse_duration_ms`, returning milliseconds, with `checked_mul` so an absurd
duration (`"999999999999999y"`) degrades to a no-op instead of wrapping into a small window. The
module header now states the unit correctly and explains why.

Both test layers were re-based onto realistic epoch-ms clocks rather than adjusted to keep passing:

- unit: a new `time_from_is_milliseconds_against_a_real_epoch_ms_clock` asserts `to - from` is
  exactly `6 * 60 * 60 * 1000` off a real `1_751_414_400_000` clock, plus
  `a_clock_smaller_than_the_override_saturates_at_the_epoch` pinning the `saturating_sub` floor
  (a wrap there would be an unbounded query, not an empty one);
- integration: the `timeShift` case now uses `[60_001, 60_040]` shifted by `"1m"` onto `[1, 40]` —
  which finds the seeded rows ONLY under ms arithmetic. Under the old seconds math the shift is 60,
  landing on `[59_941, 59_980]` and finding nothing.

## The lesson

A comment asserting a unit is not evidence of it. This one named a sibling module's contract
(`series.read`) as its justification while contradicting what that module actually enforces — and
nothing checks a prose claim. Where two modules must agree on a unit, the assertion belongs in a test
that would fail if they diverged, using values whose magnitude makes the unit unmistakable. A clock
of `100` is compatible with both readings; `1_751_414_400_000` is compatible with only one.
