# viz — a bucketed `series.read` frame collapsed to ONE blob row at the render edge

Status: **fixed** (2026-07-24, panel-resolution session, issue #101).

## Symptom

With the panel-resolution injection in place, a mode-less `series.read` chart target is upgraded to
`{mode:"buckets"}` and the tool returns `{buckets:[…N…], width_ms}`. But `viz.query`'s row-normalizer
(`viz/frame.rs::result_to_rows`) turned that whole object into a **single row** — a chart would draw one
point holding the entire buckets array, instead of N `{t,min,max,avg,last,count}` rows. The decimation
produced correct data server-side; it was invisible at the frame edge.

## Root cause

`result_to_rows` unwraps a tool's rows from a known set of plural keys (`ROW_KEYS = ["samples",
"items","rows","templates","dashboards","reminders"]`). The decimation slice (series-decimation-scope,
shipped separately) returns its rows under **`buckets`**, which was never added to `ROW_KEYS` — because
until this session nothing at the viz layer ever requested `mode:"buckets"`. So the object fell through
to the catch-all `vec![result.clone()]` (one blob row). Second, related gap: a bucket's time field is
`t`, absent from `TIME_KEYS` (`ts/time/timestamp/_time`), so even once unwrapped the frame carried no
tagged Time axis.

This is the classic "a new caller exercises an old adapter's blind spot": the adapter was exhaustive
over the callers that existed when it was written; the injection created the first caller of a shape it
didn't list.

## Fix

`viz/frame.rs`: add `"buckets"` to `ROW_KEYS` (so the frame unwraps to N bucket rows) and `"t"` to the
END of `TIME_KEYS` (last, so a row also carrying `ts`/`time` still tags that; a bucket row has only `t`).

## Regression

`crates/host/tests/viz_resolution_test.rs::wide_window_returns_bounded_buckets_not_raw_rows` asserts
`rows[0].get("t").is_some() && rows[0].get("max").is_some()` over a real seeded series through
`viz.query` — this fails against the pre-fix `ROW_KEYS` (the single blob row has no `t`/`max`), passes
after. `spike_survives_in_bucket_max_at_the_dashboard_layer` further requires the N rows to exist to find
the spike bucket.

## Lesson

A result-shape adapter keyed on a fixed list of plural keys is only exhaustive over **today's**
producers. When a new verb (or a new *mode* of an existing verb) becomes reachable at that edge, its
row-wrapper key must be added in the same change — the injection and the unwrap are one contract, not
two. A `_ => one blob row` catch-all is silent, not loud.
