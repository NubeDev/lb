# Session — rules power widgets (rules-for-widgets-scope, all three slices)

Date: 2026-07-09
Scope: [`../../../scope/frontend/dashboard/rules-for-widgets-scope.md`](../../../scope/frontend/dashboard/rules-for-widgets-scope.md)
Debug entry closed: [`../../../debugging/frontend/rules-as-source-render-path-empty.md`](../../../debugging/frontend/rules-as-source-render-path-empty.md)

## Ask

Close the rules-as-source **render** half the parent scope left blocked: a panel bound to
`{tool:"rules.run"}` must render the rule's rows through the standard read path for every view, with
read-only panel runs (no alert spam on auto-refresh) and one-line chart-shaping helpers in the cage.

## What shipped (all three slices, green)

### Slice 1 — the host render path (the unblock)

**Diagnostic first (as the scope demanded).** Wrote a failing Rust integration test
(`viz_query_test::rules_target_scalar_array_renders_rows`) that seeds a real saved rule and drives
`viz.query` over `{tool:"rules.run"}`. It returned **`rows.len()==1`, not `0`** — proving that against
the **in-process** host the recursive dispatch (`viz/query.rs::dispatch_target` →
`call_tool_at_depth("rules.run")`) **succeeds**; the whole `RunResult` collapsed to one JSON-blob row.
So Layer 1 (the reported depth>0 `Err`) did **not** reproduce here — it was spawned-gateway-harness
specific — and **Layer 2 was the entire defect** for this path.

**Fix — Layer 2 only, generically by shape** (no rules branch in the dispatcher; CLAUDE §10 held):
- `host/src/viz/frame.rs`: `result_to_rows` now calls `unwrap_rule_envelope` FIRST. A full `RunResult`
  (`{output, findings, log, ms}`) recurses into `output`; a bare `RuleOutput` is `kind`-discriminated —
  `scalar`→the value (recursed: array → N rows, non-array → one `{value}` row), `grid`→the shared
  columnar zip, `findings`/`nothing`→empty. Extracted the existing columnar-zip into `columnar_rows`
  and reused it for the grid arm (so the `kind:"grid"` key can't re-enter the envelope check and loop).
- `ui/src/features/dashboard/builder/useSource.ts`: mirrored as `unwrapRuleEnvelope` (lock-step with the
  host, per the existing comment) — so the direct-bridge path matches the `viz.query` path.
- **Un-skipped** the waiting gateway regression test in `templateView.gateway.test.tsx`
  (`renders real rows from a RULES source (rules.run)`) — green with **no template-side change**.

### Slice 2 — read-only panel runs (`route:false`)

- `host/src/rules/run.rs`: `rules_run` gains a `route: bool` param; when `false` it skips
  `route_alerts` (findings still returned in the result — honest, visible — they just don't fan out to
  the Inbox + must-deliver Outbox). `rules.run`/`rules.eval` read `route` from args (default `true`,
  existing behavior unchanged); `rules_eval` threads it too (open-question 4: exposed anywhere the args
  are composed).
- `packages/source-picker/src/sourcePicker.ts`: the Rules picker entry now emits
  `args:{rule_id, route:false}` — the host composes the flag exactly like the params form; `viz.query`
  never learns it exists. Rebuilt the package `dist` (the UI resolves the package from `dist`).
- The params form (`RuleParamsSection.tsx`) already spreads `...target.args`, so `route:false` survives
  a param edit.

### Slice 3 — chart-return helpers in the cage (`verbs/chart.rs`)

New verb family, pure compute over collected rows (a `rhai::Array` of maps — what `g.records()` or a
rows literal yields), zero authority (data-stdlib doctrine), sibling to `verbs/timeseries.rs`:
- `timeseries(rows, ts)` / `timeseries(rows, ts, keep)` — normalize the named column to canonical
  epoch-ms across the shapes sources actually return (ISO-8601 string | epoch-secs | epoch-ms), rename
  it `time`, sort ascending; the 3-arg form trims to `time` + kept columns. ISO parsing is
  dependency-free + deterministic (Hinnant's `days_from_civil`, no chrono, no wall-clock).
- `wide(rows, ts, series, value)` — long→wide pivot (multi-line shape).
- `category(rows, name, value)` — bar/pie shape (label + numeric column, validated).
Registered in `verbs/mod.rs`; catalog rows added in `catalog.rs` with a new `chart` family (added to the
two hard-coded family sets the integrity tests enforce).

## Tests (all green)

- Host unit: `viz::frame::rule_run_result_scalar_array_unwraps_to_rows`,
  `rule_output_grid_unwraps_via_columnar_path`, `rule_output_scalar_non_array_is_one_value_row`,
  `rule_output_findings_kind_is_empty` (+ the pre-existing federation/reminders cases still pass).
- Host integration (`viz_query_test.rs`): `rules_target_scalar_array_renders_rows` and the **mandatory**
  `rules_target_denied_without_run_cap_is_honest_empty` + `rules_target_workspace_isolation`.
- Route flag (`rules_test.rs`): `route_false_run_returns_findings_but_routes_nothing` (findings returned,
  **zero** inbox items + **zero** outbox effects counted in the real store); the default-routing path is
  pinned by the existing `run_rollup_alert_rule_raises_inbox_item`.
- Cage helpers (`rules/tests/chart_test.rs`, through the REAL engine): normalize+sort, ISO shape, keep-
  trim, missing-column author-error, `wide` pivot, `category` trim+non-numeric author-error; plus
  `verbs/chart.rs` unit tests for the ISO/epoch parsing.
- Gateway: the un-skipped `templateView.gateway.test.tsx` RULES case (8/8 in that file).
- `cargo build --workspace` clean; `cargo fmt` applied; UI `tsc --noEmit` clean; source-picker package
  tests 48/48.

## Notes / deviations

- **The scope's Layer 1 fix was not needed for the in-process path.** Documented in the debug entry's
  Resolution: the recursive dispatch already succeeds; only the envelope unwrap was missing. No
  tracing-span hunt was required once the failing test showed `1` not `0`.
- **Spawned-gateway path verified green (follow-up, 2026-07-09).** The earlier hedge ("if the spawned-
  gateway harness still shows an empty resolve, that is a separate cap/model-resolution issue") is now
  disproven and withdrawn. The un-skipped `templateView.gateway.test.tsx` RULES case runs under
  `pnpm test:gateway` against the REAL `test_gateway` binary — sign-in with EXACTLY
  `[mcp:rules.run:call, mcp:rules.save:call, mcp:viz.query:call, store:rule:read, store:rule:write]`,
  save the scalar-array rule, bind a panel to `{tool:"rules.run", args:{rule_id}}`, resolve via
  `viz.query`, assert 3 rows — and it is GREEN (8/8 in that file). The original "Layer 1 `Err` at the
  spawned gateway" was the Layer-2 envelope-collapse mis-read as empty, not a cap/model-resolution
  difference; the envelope unwrap alone closes the spawned path. No `viz.query` structural gap, no
  harness-specific cap issue remains.
- **Client grid columnar edge case.** `useSource.ts::toRows` has no columnar zip of its own (only
  `ROW_KEYS`); a `grid`-kind rule consumed via the *direct bridge* with array-of-array rows would not be
  zipped there. The actual render path is `viz.query` (server-shaped rows), which zips correctly — the
  client mirror is a convenience for the direct path. Left as-is; noted for a follow-up if a direct-
  bridge grid consumer appears.

## Open questions (unchanged from scope) / follow-ups

- Skill docs (`docs/skills/rules/SKILL.md` "returning chart data", `docs/skills/panels/SKILL.md` "bind a
  panel to a saved rule") — the scope names these as owned by this session; **not yet written** (code +
  tests + docs/debugging landed first). Fast-follow.
- Params-from-dashboard-variables (open Q1), findings/log in the inspector (open Q2), result caching
  (open Q3) — all deferred per the scope.
