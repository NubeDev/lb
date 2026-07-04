# Widgets — Slice C: result-render coverage (session)

- Date: 2026-07-04
- Scope: ../../scope/widgets/result-render-coverage-scope.md
- Stage: S9+ (the system-wide widget program, Slice C of `widget-platform-scope.md`)
- Status: done

## Goal

Close G1 of the widget umbrella: today only `reminder.list` declares a `descriptor.result` render
envelope; `federation.query`, `agent.invoke`, `query.*` still render via hardcoded client branches
(rich-responses follow-up #5). Slice C makes "every tool/API is a widget with a JSON schema in AND
out" true for the **tabular** host tools (`federation.query`, `query.run`) by giving each a
`result = table` envelope — so the channel CAN render them descriptor-driven, the AI discovers the
render via `tools.catalog`, and Slice B's `dashboard.pin` can pin them with ZERO tool-specific code
in the pin path. Stage exit gate (target): the headline pin of `federation.query`'s NEW envelope
works end to end against a real gateway, the pin path is generic over the tool id (rule 10), and
the descriptor-driven channel render mounts through the real `ResponseView`/`WidgetView`.

## Decisions (recorded upfront — the slice's design calls)

1. **Tool → view mapping.** `federation.query` → `table`; `query.run` → `table`. Both return the
   `{columns, rows}` columnar shape `viz::frame::result_to_rows` is written for; both are pure reads
   (no row-control write verbs → `tools[] = [<self>]`).
2. **`agent.invoke` is SKIPPED in Slice C** (deferred to Slice D, with reasoning). Its render is
   inherently streaming + nondeterministic (the run feed → durable `agent_result`); a pinned cell
   that re-runs the agent on every dashboard load is semantically wrong (cost, changing data). The
   right path is Slice D: snapshot the agent's one-shot ANSWER as a `data`-backed envelope, pin THAT.
   The shipped `kind:"agent"` palette route stays — it carries the streaming workflow a static
   descriptor cannot replace. See the scope's "Why agent.invoke is deferred" for the full argument.
3. **Keep the `kind:"query"` / `kind:"agent"` palette ROUTING branches.** Follow-up #5 conflates
   RENDERING (a tool's answer mounts as a widget — Slice C closes this half for the tabular tools)
   with ROUTING (which payload KIND the palette emits — `kind:"query"` for the async query-worker,
   `kind:"agent"` for the streaming run). The routing branches carry ASYNC/STREAMING workflow
   semantics a static descriptor cannot express; they stay. Slice C reframes follow-up #5 (rendering
   half closed for the tabular tools; routing half intentional). Nothing is DELETED; the
   descriptor-driven path is NEWLY available for the new envelopes.
4. **Panel-test-stale-view rider: LEAVE.** A pre-existing Slice A blast-radius (4 `panel_test` cases
   red on `view:"STALE"` fixtures), fully logged at
   `debugging/widgets/panel-test-stale-view-preexisting-slice-a-red.md`. Reasons not to absorb (the
   same reasons Slice B did not): it is a Slice A follow-up (the panel fixtures are library-panels
   territory, unrelated to descriptor `result` envelopes); absorbing it mixes concerns and obscures
   the trail; the fix needs care (the placeholder view must not collide with `stat`/`timeseries`/
   `gauge` used elsewhere in the test — `table` would work). The debugging entry is a complete
   playbook for the next session.
5. **`query.run` by-id pinning carries the id verbatim.** A `query.run {id:"daily"}` envelope
   captured at pin time carries `source.args = {id:"daily"}` so the pinned cell re-runs the saved
   query by id → an edit to the saved query propagates to the dashboard ("the daily query, live",
   the right mental model). Capturing the resolved text would freeze the query at pin time (NOT the
   intent). `query_run` already handles both shapes.

## What changed

### Backend (Rust)

- **`rust/crates/host/src/federation/query.rs`** — `query_descriptor()` now carries `result: Some(query_result_render())`;
  +NEW `query_result_render()` returning the `x-lb-render` table envelope
  (`{ v:2, view:"table", source:{tool:"federation.query", args:{}}, tools:["federation.query"] }`). The
  `source.tool` names the tool itself (the re-runnable read); the palette interpolates collected
  `source`/`sql` into `source.args`; `viz::frame::result_to_rows` zips the verb's `{columns, rows}` into
  named row objects (already shipped — Slice C adds no new normalization code). `tools[]` is just the
  read (a pure read has no row-control write verbs). +NEW `tests` module with
  `query_descriptor_carries_the_table_render` (the OUTPUT contract assertion).
- **`rust/crates/host/src/query/descriptors.rs`** — `run_descriptor()` now carries `result: Some(run_result_render())`;
  +NEW `run_result_render()` (same shape as `federation.query`'s). The envelope carries `{id}` verbatim
  if pinned by id → an edit to the saved query propagates to the dashboard ("the daily query, live");
  or `{lang,text,target}` for an inline one-shot (both shapes already handled by `query_run`).
  +NEW `tests` module with `run_descriptor_carries_the_table_render` and
  `save_and_compile_do_not_declare_a_render` (the explicitly-deferred tools, named not silently dropped).
- **`rust/crates/host/tests/widget_result_render_test.rs`** (NEW) — 8 integration tests through the REAL
  MCP bridge (`call_tool`) + the direct shell path (`dashboard_pin`), against a real `Node::boot` store:
  1. `catalog_serves_the_new_result_envelopes_to_a_granted_caller` — both envelopes reach `tools.catalog`.
  2. `catalog_hides_the_result_envelope_when_the_tool_cap_is_absent` — the menu IS the permission model
     (a caller without `mcp:federation.query:call` gets neither the descriptor NOR its envelope).
  3. `pin_federation_query_envelope_persists_and_reloads_intact` — the HEADLINE: pin the envelope →
     `dashboard.get` → `pin-federation-query` cell with `view`/`source.tool`/`source.args` intact, ZERO
     federation-specific code in the pin path.
  4. `pin_path_is_generic_over_an_arbitrary_tabular_tool_id` — re-assert rule 10 (a `__test__.*` tool
     mints identically).
  5. `pin_in_ws_a_is_invisible_to_ws_b_for_federation_query` — workspace isolation at the pin/persist
     layer (the wall is structural; the cell's `source.tool` re-resolves under the viewer's grant at
     render via `federation/query.rs:42`'s namespace-wall).
  6. `shell_path_and_headless_mcp_call_produce_the_same_federation_query_cell` — the SAME cell from
     `dashboard_pin` and `call_tool` → `dashboard.pin`.
  7. `query_run_envelope_mints_a_table_cell_with_the_captured_id` — `pin-query-run` with `source.args.id="daily"`.
  8. `re_pin_federation_query_replaces_in_place_not_duplicates` — idempotency for the new envelope.

### Frontend (UI)

- **`ui/src/features/channel/ResponseViewResultRender.gateway.test.tsx`** (NEW) — 3 real-spawned-gateway
  tests proving the descriptor-driven channel render path works for the new envelopes. The HEADLINE: a
  `rich_result` carrying the `federation.query` envelope MOUNTS through `ResponseView` (NOT `QueryCard`)
  — the `PinToDashboard` affordance is the structural marker (only ResponseView mounts it). Plus the
  `query.run` parity and an arbitrary-unknown-tool-id envelope (rule 10: the render path is tool-agnostic).
  Mounts `<MessageItem>` directly (the routing decision under test) — does NOT render the full
  `<ChannelView>`, so it does not hit the pre-existing `useTheme` red (see Pre-existing reds).

**No production UI code changed.** Slice C is backend config + tests; the existing `ResponseView`/
`WidgetView`/`PinToDashboard` already consume any envelope. The UI gateway test is the parity proof.

## Tests

### Rust descriptor units

```
$ cargo test -p lb-host --lib descriptors query_descriptor_carries
test federation::query::tests::query_descriptor_carries_the_table_render ... ok
test query::descriptors::tests::run_descriptor_carries_the_table_render ... ok
test query::descriptors::tests::save_and_compile_do_not_declare_a_render ... ok
test result: ok. 3 passed; 0 failed
```

### Rust integration (the HEADLINE)

```
$ cargo test -p lb-host --test widget_result_render_test
test catalog_hides_the_result_envelope_when_the_tool_cap_is_absent ... ok
test pin_in_ws_a_is_invisible_to_ws_b_for_federation_query ... ok
test pin_path_is_generic_over_an_arbitrary_tabular_tool_id ... ok
test shell_path_and_headless_mcp_call_produce_the_same_federation_query_cell ... ok
test query_run_envelope_mints_a_table_cell_with_the_captured_id ... ok
test pin_federation_query_envelope_persists_and_reloads_intact ... ok
test catalog_serves_the_new_result_envelopes_to_a_granted_caller ... ok
test re_pin_federation_query_replaces_in_place_not_duplicates ... ok
test result: ok. 8 passed; 0 failed; 0 ignored
```

### Rust no-regression (Slice A + Slice B unchanged)

```
$ cargo test -p lb-host --test widget_catalog_test --test widget_pin_test
widget_catalog_test:  test result: ok. 8 passed; 0 failed
widget_pin_test:      test result: ok. 10 passed; 0 failed
```

### UI gateway (the descriptor-driven render path)

```
$ pnpm test:gateway src/features/channel/ResponseViewResultRender.gateway.test.tsx
 ✓ src/features/channel/ResponseViewResultRender.gateway.test.tsx (3 tests) 80ms
 Test Files  1 passed (1)
      Tests  3 passed (3)
```

### UI unit + sibling gateway (no regression)

```
$ pnpm test
 Test Files  94 passed (94)
      Tests  561 passed (561)

$ pnpm test:gateway src/features/channel/palette/CommandPalette.reminders.gateway.test.tsx \
                   src/features/channel/PinToDashboard.gateway.test.tsx
 CommandPalette.reminders.gateway.test.tsx: 11/11 green
 PinToDashboard.gateway.test.tsx: 4/4 green
```

### `cargo fmt` clean, `cargo build --workspace` clean.

## Pre-existing reds (NOT this slice's)

- `panel_test` 4 cases (`ref_hydrates_coexists_propagates_and_ignores_echoed_spec`,
  `cross_ws_ref_rejected_and_dangling_placeholders`, `delete_refused_while_in_use_unless_forced`,
  `dashboard_save_returns_hydrated_ref_cells`) — confirmed red on the clean tree before this slice's
  changes (`cargo test -p lb-host --test panel_test` → 6 passed, 4 failed, PIPESTATUS=101). Logged at
  `debugging/widgets/panel-test-stale-view-preexisting-slice-a-red.md`. Slice C does NOT touch
  `panel_test.rs` or `dashboard/save.rs`/`views.rs`/`genui.rs`/`bounds.rs`/`panel/validate.rs` — the
  validator's behavior is unchanged.
- `CommandPalette.gateway.test.tsx` (6 cases) + `CommandPalette.agent.gateway.test.tsx` (2+ cases) —
  fail with `useTheme must be used within ThemeProvider` (from `src/lib/theme/useTheme.ts:8`), raised
  during the `<ChannelView>` render. Surfaced during this slice; NOT this slice's regression (Slice C's
  only UI change is the additive `ResponseViewResultRender.gateway.test.tsx` which mounts
  `<MessageItem>` directly, never `<ChannelView>`; vitest runs each test file in isolation with its own
  module registry, so a new test file cannot break unrelated ones; both failing files fail IDENTICALLY
  in isolation). Cause: in-flight motion/theme work uncommitted in the working tree before Slice C
  started (`git status` showed pre-existing mods to motion consumers + `panel-builder` + flows rename +
  `viz.phase3.gateway.test` + `data-studio-ux` + `useSource`/`useVizQuery`). The four sibling gateway
  tests that mount `<MessageItem>` directly — `PinToDashboard.gateway` 4/4,
  `CommandPalette.reminders.gateway` 11/11, and Slice C's new `ResponseViewResultRender.gateway` 3/3 —
  are GREEN. Logged at `debugging/frontend/channel-palette-gateway-useTheme-not-in-provider.md` (a
  follow-up for the in-flight motion/theme owner).
- `agent_routed_test::an_edge_invokes_the_hub_agent_over_the_routed_namespace` — failed ONCE under
  `cargo test --workspace` (parallel execution) but PASSES in isolation (3/3 in
  `cargo test -p lb-host --test agent_routed_test`). Confirmed parallel-execution flake (the routed
  Zenoh namespace + agent substrate is timing-sensitive under load), NOT this slice's regression.

## Definition of done

Per HOW-TO-CODE.md §5. Checked:

- [x] The work satisfies the scope (envelopes on `federation.query` + `query.run`; headline pin;
      descriptor-driven channel render).
- [x] Per-tool descriptor unit tests (the envelope shape) — green.
- [x] Capability-visibility (the catalog gate) + workspace-isolation tests for the new envelopes — green.
- [x] The HEADLINE Rust integration test: pin `federation.query`'s envelope via `dashboard.pin` →
      persisted cell that reloads — green.
- [x] UI gateway test: post the descriptor's `result` → mounts through `ResponseView`/`WidgetView` — green.
- [x] Slice B `widget_pin_test` (10/10) + Slice A `widget_catalog_test` (8/8) stay green.
- [x] `cargo build --workspace` + `cargo fmt` clean; `pnpm test` (unit) 561/561 green.
- [x] No mock data / no fake backend (rule 9) — real `Node::boot` store, real spawned gateway.
- [x] No core branch on a tool id (rule 10) — the mint path stays generic; re-asserted by
      `pin_path_is_generic_over_an_arbitrary_tabular_tool_id`.
- [x] `docs/scope/widgets/result-render-coverage-scope.md` written + this session log filled.
- [x] SKILL.md updated with the list of tools that declare a `result` render today (grounded in a live
      `tools.catalog` run — the integration test `catalog_serves_the_new_result_envelopes_to_a_granted_caller`
      asserts the exact JSON shape against a booted node).
- [x] STATUS.md Slice C row added (building → shipped).
- [x] `public/frontend/dashboard.md` Slice C section added.
- [x] `channels-rich-responses-scope.md` follow-up #5 reframed (rendering half closed for tabular tools;
      routing half intentional).
- [x] Umbrella `widget-platform-scope.md` Slice C section + G1 + the table row #2 refreshed.

## Cross-links

- **Scope:** `scope/widgets/result-render-coverage-scope.md` (NEW — written before building). The
  umbrella's Slice C section is reframed (sketch → shipped); G1 is partially-closed; the table row #2
  status is updated. The umbrella's open question "Which generative-UI language for the AI `view`?"
  is untouched; the per-widget-version-consumption open question is untouched.
- **Public:** `public/frontend/dashboard.md` gains a "Result-render coverage (Slice C)" section.
- **STATUS.md:** the Slice C row added (building → shipped); Slice B demoted to "Just shipped" (kept).
- **Skill:** `skills/dashboard-widgets/SKILL.md` gains "Which tools declare a `result` render today"
  (the list — `reminder.list` + `federation.query` + `query.run`; `agent.invoke` deferred with reasoning).
- **Sibling scopes refreshed:** `scope/channels/channels-rich-responses-scope.md` follow-up #5 reframed.

## Notes for the next slice (Slice D)

- **`agent.invoke`'s `result` envelope belongs to Slice D**, not Slice C. Slice D's job: snapshot the
  agent's one-shot ANSWER into a `data`-backed envelope (NOT `source`-rerun — re-running the agent on
  every dashboard load is semantically wrong) and pin THAT. The pinned widget shows the captured answer;
  the render surface is stable. The shipped `kind:"agent"` palette route carries the streaming workflow
  and stays.
- **`query.save`/`query.compile`** — `query.save` is a write verb (its answer is the saved record, not a
  render); `query.compile` returns SQL text (marginally useful as a `code` view). Named follow-ups.
- **Per-widget version stamping** (Slice A follow-up) still deferred.
- **The `panel_test` "STALE" fixtures** (pre-existing Slice A red) — `debugging/widgets/panel-test-stale-view-preexisting-slice-a-red.md`.
  A Slice A follow-up: change the echoed-spec placeholder from `view:"STALE"` to a real-but-different
  view (a safe pick is `table` — it's not used elsewhere in `panel_test.rs`'s propagation assertions,
  which use `stat`/`timeseries`/`gauge`).
- **The `CommandPalette.gateway`/`agent.gateway` `useTheme` red** — `debugging/frontend/channel-palette-gateway-useTheme-not-in-provider.md`.
  A follow-up for the in-flight motion/theme owner: wrap `<ChannelView>` in a `<ThemeProvider>` in the
  two failing gateway tests, OR make `Reveal`/`useMotionPref`'s `useTheme` call optional
  (`useThemeOptional()` pattern).

## Status: done
