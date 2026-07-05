# Session — persona run only sees 3 tools (tools.catalog starved the agent menu)

**Date:** 2026-07-05 · **Branch:** `insights-v1` · **Area:** agent run assembly / tools.catalog

## The ask

Under `builtin.widget-builder` (extends `builtin.data-analyst`) on `/dashboards`, the in-house
agent (Z.AI GLM-4.6) reported only `dashboard.catalog`, `dashboard.pin`, `federation.query` and said
`datasource.list` was unavailable. Root-cause and fix in the right layer + regression test.

## Investigation path

1. Read the suspected persona layer (`personas/resolve.rs`, `personas/apply.rs`, `menu.rs`,
   `dispatch.rs`) — all correct on inspection: `resolve_effective` BFS-unions the `extends` closure,
   `narrow_tools` glob-intersects, `dispatch.rs` narrows `reachable ∩ persona`.
2. Proved it with a test: on a fresh node + `seed_personas`, widget-builder's effective
   `granted_tools` contains `datasource.list` and narrowing keeps the parent surface. **Persona layer
   exonerated before touching it.**
3. Checked the cap wall: `member_caps()` grants `mcp:datasource.list:call` — not a grant gap.
4. Followed the menu to its head: `reachable_tools` = `tools_catalog` = `host_descriptors()`
   (the ~11 guided-palette verbs) + extension registry. **Gap 1:** the whole rest of the host-native
   verb surface was structurally absent from every agent menu. The live 3-tool menu is exactly
   `palette subset ∩ widget-builder globs`.
5. The intended full inventory exists (`system/catalog.rs::HOST_TOOLS`, served by `system.tools`) —
   but **gap 2:** it was missing the `datasource.`/`viz.`/`flows.`/`rules.`/`query.`/… families; its
   coverage test iterated a stale hand-copied prefix list instead of the dispatcher's own.

## Fix (3 files)

- `rust/crates/host/src/tools/catalog.rs` — after the rich descriptors, serve the full host-native
  inventory: name-only `ToolDescriptor`s from `system::host_catalog()`, deduped against descriptors,
  filtered to `tool_call::is_host_native` (only what the MCP bridge can dispatch), gated per verb by
  the same `authorize_tool`. Rejected alternative: a second enumeration inside `reachable_tools` —
  that forks "what can I run" into two truths; the catalog is the documented honest menu.
- `rust/crates/host/src/system/catalog.rs` — added the ~70 missing `HOST_TOOLS` rows; coverage test
  now derives from the dispatcher's consts (+ `system.`).
- `rust/crates/host/src/tool_call.rs` — `is_host_native` now iterates shared
  `HOST_NATIVE_PREFIXES`/`HOST_NATIVE_EXACT` consts (single source for dispatcher + coverage test).

Known limitation: inventory rows carry no arg schema (a verb gains one by adding a palette
descriptor) — the model can call them but may guess arg names.

## Tests (green)

- NEW `rust/crates/host/tests/persona_menu_full_catalog_test.rs`
  (`reachable_tools_serves_full_host_inventory_and_persona_keeps_it` — includes the deny half: an
  ungranted verb stays absent; `widget_builder_unions_data_analyst_surface`).
- All catalog/persona/agent suites re-run green: `tools_catalog_test`, `agent_persona_test`,
  `agent_persona_catalog_test`, `agent_persona_coding_test`, `system_map_test`,
  `agent_in_house_wiring_test`, `agent_def_test_test`, `agent_runtimes_test`, `reminder_fire_test`,
  `widget_result_render_test`; full `cargo test --workspace` (modulo the known pre-existing
  `agent_routed_test` flake noted in memory/debugging).

## Debug history

`docs/debugging/agent/persona-menu-missing-tools-catalog-descriptor-only.md` + README row.

## Round 2 (same session): empty answer + invisible tool calls

Live testing after the catalog fix surfaced two more issues (transcript provided by the user):
"add a widget…" settled with an **empty** answer, and the dock never showed which tools a run
called. Root-caused and fixed:

- `run.rs`: the answer was the LAST turn's content even when empty (final empty `done` turn, or a
  silent MAX_STEPS ceiling exit mid-tool-work). Now: last **non-empty** content wins, and a ceiling
  exit appends an honest `[stopped at its 8-turn ceiling …]` note. Tests:
  `agent_answer_fallback_test.rs` (2).
- Dock UI: `DockRunStatus` renders the run's tool calls as a live ✓/✗/spinner list that stays
  visible after the run settles (`AgentDock` keeps the strip mounted when tools were captured);
  `exportTranscript` appends the latest run's captured calls. Tests: `DockRunStatus.test.tsx` (+3),
  `exportTranscript.test.ts` (+2); full `pnpm test` 650/650.
- Debugging entry:
  [`debugging/agent/run-answer-empty-last-turn-content-overwrites.md`](../../debugging/agent/run-answer-empty-last-turn-content-overwrites.md).
- Follow-up recommendation: `MAX_STEPS = 8` is tight for builder personas; a per-workspace/persona
  ceiling is a recorded scope follow-up. Tool calls in the durable channel record (not just the live
  feed) is a second follow-up.

## Round 3 (same session): `information_schema` probes + guessed tables, then the ceiling

The next live transcript showed the model probing `SELECT … FROM information_schema.tables` via
`federation.query` (cryptic DataFusion "table not found" — catalog schemas are unreachable by
design) and then **guessing** a table name (`meter_readings`). Root-caused and fixed:

- **Steering, both layers:** the host gate (`federation/validate.rs`) and the sidecar's parser
  (`extensions/federation/src/validate.rs`) now reject catalog-schema SQL with a message naming
  `federation.schema` and its `{source, table?}` args. Steering tests in both — green.
- **`federation.schema` got a real arg-schema descriptor** (`federation/schema.rs` →
  `tools/descriptor.rs`); it had been advertised name-only, so the model couldn't form the call.
- Debugging entry:
  [`debugging/agent/federation-information-schema-probe-cryptic-plan-error.md`](../../debugging/agent/federation-information-schema-probe-cryptic-plan-error.md).

**Verified live (headless, `POST /agent/invoke`, GLM-4.6, widget-builder):** the retest run led with
`datasource.list → federation.schema`, issued zero `information_schema` probes and zero guessed
table names, and the answer carried the honest ceiling note instead of coming back empty. A direct
`federation.query` with `information_schema.tables` returned the steering message.

## Round 4 (same session): the ceiling itself — MAX_STEPS 8 → 16

Both verification runs died at the 8-turn ceiling mid-build (the honest note, never a finished
widget) — the build path (orient → `federation.schema` → probe queries with one real-error retry →
`viz.query` → `dashboard.save`) measured 10–14 turns live. `MAX_STEPS` raised 8 → 16 in `run.rs`;
the per-workspace/persona ceiling stays the recorded scope follow-up. Entry:
[`debugging/agent/run-ceiling-too-low-for-builder-personas.md`](../../debugging/agent/run-ceiling-too-low-for-builder-personas.md).

## Live-node note

The dev node binary does not hot-reload — a running node older than this fix still serves the
starved catalog (`make kill && make dev`).
