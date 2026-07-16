# Single scan page drops rows past 200 — values missing / runs never finalise after deploy

- Area: flows
- Status: resolved
- First seen: 2026-07-15 (latent since the run-store shipped; only reproduces once a workspace's
  flows tables outgrow one scan page — which is why fresh test suites stayed green while a deployed
  node got flaky)
- Resolved: 2026-07-15
- Session: ../../sessions/flows/flows-readback-hardening-session.md
- Regression test: rust/crates/host/tests/flows_scan_paging_test.rs
  (`node_state_reads_values_past_one_scan_page`,
  `run_finalizes_and_reports_steps_past_one_scan_page`)

## Symptom

On a deployed (long-lived) node, flow value read-back is intermittently wrong: `flows.node_state`
paints some nodes `null` although they ran; `flows.runs.get` shows a run with missing steps; runs
sometimes stay `pending` forever (never finalise) or a `Skip`-concurrency flow fires overlapping
runs. Everything is consistently green on a fresh store and in tests.

## Reproduce

Seed 200+ rows into any shared flows table (`flow_node_state`, `flow_step_output`, `flow_run`,
`flow_input`, `flow`) whose ids sort before the flow/run under test, then read it back:

- 240 `flow_node_state` rows for other flows → `flows.node_state {id}` returned `null` values for a
  flow whose records sit past the first page.
- 240 terminal `flow_step_output` rows of an old run → a new run's own slots fall past the page;
  `scan_run_slots` missed them, so `ready_slots` lost the frontier and `finalize_if_complete`
  either never fired (run stuck `pending`) or mis-settled.

## Investigation

Traced the "not 100% consistent getting values back after deploy" report end to end. The read-back
contract (flow-persistent-runtime-scope: `flow_node_state` last-value + `flows.node_state`) was
implemented correctly per record — but every flows verb that reads a **shared ws table and filters
in code** called `lb_store::scan(…, MAX_SCAN_LIMIT, None)` **once** and never followed `page.next`.
`lb_store::scan` hard-caps a page at 200 rows by design (the DB-browser grid contract). Ten call
sites had the pattern: `node_state.rs` (×2), `run_store.rs` (`scan_run_slots`, per-port retained
inputs, `merged_params_with_inputs`), `runs.rs` (×2), `save.rs` (`flows_list_internal` — also feeds
the cron/source reactors and the orphan sweep), `concurrency.rs`, `orphan_sweep.rs`.

## Root cause

`lb_store::scan` is deliberately one bounded page with a cursor; the flows verbs treated one page as
"the whole table". Below 200 rows per table the two are identical — every test and young deployment
passes — and past 200 rows later-sorting records silently vanish from reads, the drive loop, and
finalisation.

## Fix

`rust/crates/host/src/flows/scan_all.rs` — the cursor loop (`scan_all`) that drains every page —
and every one of the ten call sites moved onto it. Full-drain-then-filter (not a prefix-seeded
early exit) because the scan cursor is the SurrealDB `<string>id` rendering (`⟨⟩`-bracketed), whose
ordering does not agree with the display id; the flows tables are retention-bounded so the drain
stays small. A store-level prefix scan is the named follow-up if one profiles hot.

## Lesson / prevention

A paged API used as if it were a full read is invisible until production data outgrows one page —
tests must seed **past the page boundary** when the code filters a shared table in memory. The same
single-page pattern existed outside flows (`rules/get.rs`, `dashboard/store.rs`, `panel/store.rs`,
`report/store.rs`, `nav/store.rs`, `brand/store.rs`, `render_templates/store.rs`, `insight/notify.rs`)
— swept the same way in the follow-up
([#69](https://github.com/NubeDev/lb/issues/69), resolved 2026-07-15): those sites (and the flows
drain) now share one canonical `lb_store::scan_all`. See
[../store/single-scan-page-drops-rows-past-200-non-flows.md](../store/single-scan-page-drops-rows-past-200-non-flows.md).
