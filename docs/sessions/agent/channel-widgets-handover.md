# HANDOVER — GenUI widget previews in the agent dock (channel-widgets)

**Date:** 2026-07-06 · **Branch:** `insights-v1` (uncommitted work) · **Workspace under test:** `acme`, user `user:ada`

## The goal

From the agent dock: ask a data question → the agent posts a **live widget preview into the
conversation** (`rich_result` channel item), including composed **GenUI** layouts bound to real
queries — and the user can pin it to a dashboard. Preview must NEVER `dashboard.save`.

## Current state — ONE bug left

The pipeline works end-to-end EXCEPT the final render: the model emits a **wrong IR dialect**.

Live evidence (latest dock run, widget-builder persona): agent ran `dashboard.catalog` →
`federation.query` (proved the SQL) → `channel.post` ✓. The posted envelope is
`view:"genui"` with `options.genui.ir` shaped like:

```json
{ "components": { "root": { "type": "stack", ... } } }
```

Defects vs the real IR (`packages/genui/src/ir/types.ts`):
- uses `"type"` instead of `"component"` per component;
- no per-component `id` field;
- missing `ir.v` (must be `1`) and missing `surface: { "surfaceId": "...", "root": "<root id>" }`.

So `GenUiView` (ui/src/features/dashboard/views/genui/GenUiView.tsx) shows its invalid/draft
state in the dock instead of the widget. **Nothing validates a rich_result at post time** — unlike
`dashboard.save`, which runs `check_genui_cells` and whose loud errors let the model self-correct
(that self-correction visibly worked in an earlier run).

## Recommended next step (do this)

1. **Validate genui rich_result bodies at `channel.post`, host-side.** In
   `rust/crates/host/src/channel/tool.rs` (or `post` path): when the posted body parses as
   `kind:"rich_result"` with `view:"genui"`, run the SAME structural checks as
   `rust/crates/host/src/dashboard/genui.rs::check_genui_cells` (ir is object, numeric `v`,
   `components` map, names resolve in `genui_catalog.json`, root defined, ≤8KB) and return a
   loud `BadInput` naming the defect (e.g. "component `root`: use `component`, not `type`;
   missing `surface.surfaceId`"). The agent loop feeds tool errors back — the model fixes itself.
2. **Tighten the skill** `docs/skills/channel-widgets/SKILL.md`: add a "common mistakes" block —
   `component` not `type`; every component repeats its `id`; `ir.v: 1` + `surface` required;
   controls (slider/button) need an `action` wired to a tool or they are decorative.
3. Optional UI defense: run `@nube/genui` normalize/validate warnings in `ResponseView` for genui
   payloads and render the warnings honestly.
4. Retest in the dock (restart node first): ask
   *"use this query to build a genui widget … add a preset limit slider and a run button"* —
   expect a rendered GenUI card, then Pin it and confirm the dashboard cell renders with data.

## What already shipped today (all tested green)

Slice 1 — agent posts widgets:
- `agent/personas/personas.toml` — data-analyst absorbed the widget surface (user decision:
  no persona-switching): + `channel.post`, `dashboard.pin`, `dashboard.*`, `panel.*`,
  `layout.get/set`, `template.*`; grounding = datasources/query/store-read/**channel-widgets**/
  **genui-widget**/**dashboard-widgets**. Both personas' identities: "a posted widget IS the
  preview — dashboard.save/pin only when asked" (fixes the 6× failed dashboard.save loop).
- `channel/agent_worker.rs` — run goal now ends with `[conversation channel: <cid>]`.
- `docs/skills/channel-widgets/SKILL.md` → seeded `core.channel-widgets` (corpus auto-embeds
  from docs/skills/*/SKILL.md via lb-assets build.rs). Added to
  `workspaces/default_skills.rs::DEFAULT_CORE_SKILLS`.

Slice 2 — genui envelopes:
- `channel/payload.rs` — `RichResultPayload.sources` (opaque Value; v3 Target list) +
  UI mirror `ui/src/lib/channel/payload.types.ts`.
- `ui/src/features/channel/ResponseView.tsx::buildCell` — declared `sources[]` first/un-hidden;
  no duplicate hidden leash entries. Mirrored in `rust/.../dashboard/pin.rs` (pin carries
  `sources[]` verbatim — it used to drop them).
- `ui/src/features/dashboard/views/genui/genuiData.ts::genuiTargets` — hidden-only sources[]
  no longer shadow the v2 single `source` (promoted to refId `A`).

Also earlier today (separate fix, shipped): `agent/run.rs` one-shot **nudge** — a `done` turn
with empty content after tool work re-asks the model once instead of settling on the preamble
(`docs/debugging/agent/run-finished-empty-after-tool-work-answers-with-preamble.md`).

## Tests / commands

- `cd rust && cargo test -p lb-host --test channel_agent_worker_test` — 10 ✓ (incl. NEW
  rich_result post happy-path + cid-in-goal + ws-isolation; channel.post capability-deny).
- `cargo test -p lb-host --test widget_pin_test` — 11 ✓ (incl. NEW genui sources carry-through).
- `cargo test -p lb-host --test agent_answer_fallback_test` — 3 ✓ (nudge).
- Persona/skill/dashboard suites ✓; `cargo build -p node` ✓.
- `cd ui && pnpm vitest run src/features/dashboard/views/genui/genuiData.test.ts
  src/features/channel/ResponseView.test.tsx` — 16 ✓; `tsc --noEmit` ✓.

## Live-environment notes (important)

- **The dev node must be restarted** to pick up personas/skill/worker changes:
  `make kill && make dev` (node never hot-reloads; this bit us twice today).
- `core.channel-widgets` / `core.genui-widget` / `core.dashboard-widgets` were **granted to the
  existing `acme` workspace manually** (default grants apply only at workspace creation):
  `POST /skills/{id}/grant` as ada → 204 ×3. Fresh workspaces get them automatically.
- Dock render path (all pre-existing, unchanged): `MessageList → MessageItem(kind rich_result)
  → ResponseView.buildCell → WidgetView → GenUiView`; Pin button = `PinToDashboard` →
  `dashboard.pin`.
- Known cosmetic: `exportTranscript` shows a rich_result as raw JSON (follow-up in scope doc).

## Docs

- Scope: `docs/scope/channels/channel-widgets-scope.md` (decisions + follow-ups).
- Session: `docs/sessions/agent/channel-widgets-session.md`.
- Debugging (earlier fix): `docs/debugging/agent/run-finished-empty-after-tool-work-answers-with-preamble.md`.
