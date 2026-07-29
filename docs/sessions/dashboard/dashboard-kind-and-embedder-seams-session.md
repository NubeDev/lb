# Session — `Dashboard.kind`, `report.export` on the A4 grid, and three embedder seams

Date: 2026-07-29. Branch `feat/report-kind-dashboards`.

Scopes: [`../../scope/dashboard/dashboard-kind-scope.md`](../../scope/dashboard/dashboard-kind-scope.md)
and [`../../scope/dashboard/embedder-outbox-and-service-token-scope.md`](../../scope/dashboard/embedder-outbox-and-service-token-scope.md).
Downstream consumer: `NubeIO/rubix-ai`, which rebuilds its whole reports surface on this.

## Ask

Two things a downstream product host needed, and one it turned out nobody had:

1. Make a **report a dashboard** — a typed `kind` on the record — and re-address `report.export` at
   it, composing A4 pages from the cell grid instead of the linear `blocks[]` notebook.
2. Give an embedder the three seams a headless report renderer needs: its own **outbox target**, a
   **short-lived service session** for a non-interactive worker, and re-exports so it can name those
   types with only the `lb-node` dep.
3. (Found while building 2.) Actually **spawn the reminder reactor**.

## What shipped

### `Dashboard.kind` — the `width` pattern, plus the roster

`crates/host/src/dashboard/model.rs` — the field with `null_default`, `KIND_DASHBOARD`/`KIND_REPORT`,
`Dashboard::is_report()` as the ONE predicate (empty reads as `"dashboard"`, so nothing migrates), and
`kind` on `DashboardSummary` + its `From<&Dashboard>` projection. That last part is the one place this
differs from `width`, and it is the reason `kind` is a typed field at all: the roster is where the two
kinds get partitioned, and doing it any other way costs a full `dashboard.get` per row.

`save.rs` — descriptor property, the `dashboard_save_meta` parameter, the prev-value tuple, the
`unwrap_or(prev_kind)` that makes it preserve-on-omit, and `check_kind`. Validated where `width` is
not, deliberately: an unknown `width` degrades to the default layout and is *visible*; a mistyped
`kind` drops the record out of both rosters, so it saves "successfully" and is then findable nowhere.

`tool.rs` + `role/gateway/src/routes/dashboard.rs` — the MCP arg and the REST field, both preserving
present-vs-absent. Then the four sites that are easy to miss: the `pin.rs`/`closure.rs` struct
literals, and the positional `dashboard_save_meta` calls in `grafana/import.rs` (passes `None`, so
re-importing over a report never demotes it) and `pack/apply.rs` (reads `kind` from the pack JSON, so
a pack can declare a report).

### `report.export`, re-addressed

`report/export.rs` (197 → 140 lines) now reads through `dashboard_get`, so the same three gates
re-run under the exporter, and **refuses a non-report dashboard**: a 12-column board authored for a
wide screen, laid onto a 166 mm page, is not a report but a broken PDF.

`report/compose.rs` (new) owns page composition: sort cells into reading order (`(y, x)` — the stored
array is in *save* order, which is not visual order after a drag), band them onto pages by grid row,
pair each with its capture. A cell whose capture is missing **or empty** is still placed, as an error
tile. Empty pages between occupied ones are kept, because that is where the author put the content.

`crates/render/` gained `geometry.rs` (the A4 numbers, and react-grid's own arithmetic evaluated in
millimetres) and `place.rs` (absolute `#place` boxes), plus `Assembled.placements` — positionally
aligned with `pages`, empty ⇒ the shipped markdown path.

**A pre-existing drift this exposed and fixed:** the deleted UI's `a4-sheet.ts` claimed a uniform
20 mm margin while the Typst template has always used x 22 / top 24 / bottom 22. Nothing caught it
because nothing laid anything out *by position*. `geometry.rs` now states the true numbers and pins
them with a round-trip test the shell's own test asserts against.

### The three embedder seams

- `BootConfig.outbox_providers.targets` — a generic registry folded into the boot `RouterTarget`
  after the built-ins (so a host can add a target *or replace one*). `RouterTarget` now stores `Arc`
  and gained `route_dyn` for a caller that cannot name the concrete type.
- `mint_full_session_with_ttl` — the shipped `mint_full_session` with the TTL lifted to a parameter,
  the original delegating at the 12-hour constant. Plus `RunningNode::mint_service_session`, which
  returns `None` on a headless node.
- `node/src/lib.rs` re-exports `Principal`, `Store`, `Target`/`DynTarget`/`OutboxEffect`,
  `enqueue_outbox` and the asset verbs, under the `BrowserSessionConfig` precedent.

`lb_host` also gained `pub use lb_outbox::Effect as OutboxEffect` — **aliased**, because the crate
root's `Effect` is the *agent's* effect. Two unrelated types with one obvious name; an embedder
implementing `Target` must be able to name the right one.

### The reminder reactor, which had never been spawned

`crates/host/src/reminder/spawn.rs` + the wiring in `node/src/reactors.rs`. `react_to_reminders`
shipped, is tested, is exported — and `reactors.rs` spawned flow, agent, approval, relay, ingest,
retention, compaction and insight-digest ticks, but not it. On any node booted through `boot_full`, a
user could author a cron schedule, see it listed with a "next run" time, and have it never once fire.
The same missing-driver class as the ingest drain and the retention GC, both already recorded in that
same file with the same "previously never booted" note.

Two details worth keeping: the cadence is 10s because cron resolves to the minute and `advance()`
does **not** backfill (a missed slot is skipped, not deferred), and it feeds a **seconds** clock, not
the millis several sibling reactors use — the reminder plane is a logical second clock, and millis
would put every `next_attempt_ts` ~55,000 years in the past and fire everything at once.

## Tests

`cargo check --workspace --all-targets` clean (after `make build-wasm` — `test-be-no-wasm-dep`).

```
$ cargo test -p lb-render
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

$ cargo test -p lb-host --test dashboard_test
test dashboard_kind_round_trips_preserves_and_validates ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s

$ cargo test -p lb-host --test report_test
test export_composes_a_report_kind_dashboard_to_pdf ... ok
test export_is_denied_without_the_export_cap_and_without_dashboard_read ... ok
test export_is_workspace_isolated ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

$ cargo test -p lb-host --test reminders_reactor_test
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.11s

$ cargo test -p lb-role-gateway --test report_routes_test
test export_returns_pdf_bytes ... ok
test exporting_a_plain_dashboard_is_refused ... ok
test an_unknown_dashboard_kind_is_refused_at_save ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.29s

$ cargo test -p lb-node --test embedder_seams_test
test an_embedder_registered_outbox_target_receives_its_effects ... ok
test a_due_reminder_fires_from_the_boot_spawned_tick ... ok
test a_service_session_is_short_lived_and_carries_only_the_principals_caps ... ok
test a_headless_node_mints_no_service_session ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.25s
```

The mandatory categories are covered by name: **capability-deny** both ways in
`export_is_denied_without_the_export_cap_and_without_dashboard_read` (missing `report.export`, and
missing `dashboard.get`, each with a passing negative control so neither deny is a tautology), and
**workspace-isolation** in `export_is_workspace_isolated`.

The `node` seam tests assert the *boot-spawned* reactors, not the passes: nothing in them calls
`relay_outbox` or `react_to_reminders`. That distinction is the entire point — the passes were
already green while the drivers did not exist.

## `report_test.rs` was 8/9 RED on master, and is now green

Verified, not assumed: `report_test.rs` was run in a detached worktree at pristine `c05ebb75` and
8 of its 9 tests failed there, every one on `Store(Decode("invalid workspace id: \"ws:acme\""))`.
A colon is not legal in a workspace id — the store's `USE NS` validation, from the `node-v0.11.0`
per-query namespace work — and this file still used `ws:acme` throughout. It was part of lb's
recorded failing-test baseline. Since the export tests in it were being rewritten anyway, the file's
workspace ids were hyphenated; all 11 now pass.

## Follow-ups (named, not done)

- **Retiring the `report.*` notebook verbs** (`save`/`get`/`list`/`share`, the `Report` record, its
  catalog rows and role caps). Now genuinely unused by the shipped consumer, but deleting them is a
  breaking release on its own; it is housekeeping, not part of this.
- **`BootConfig` has no API-key pepper**, so an embedded node generates a fresh random one per
  process and every API key it issues is silently invalidated on restart. Found while investigating
  the token mint; out of scope here, worth its own fix.
- **`relay.rs` discards the delivery error string** (`Err(_reason)`), so a dead-lettered row carries
  no explanation — including the "no delivery adapter registered" case. The new target registry makes
  that less likely to be hit, and no less opaque when it is.
