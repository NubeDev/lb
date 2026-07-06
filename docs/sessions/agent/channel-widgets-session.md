# Session — channel widgets: live widget previews in the agent dock

**Date:** 2026-07-06
**Scope:** `docs/scope/channels/channel-widgets-scope.md` (written this session, shipped v1)

## Ask

Query the DB from the agent dock and get a rendered widget — GenUI/OpenUI included — as a
**preview in the conversation**, with the option to save it as a dashboard widget/panel. Live
failure observed: the widget-builder persona kept driving `dashboard.save` (six failed attempts)
instead of previewing. Second user decision: merge the widget surface into the data-analyst
persona (no more persona-switching between "data" and "widgets").

## What changed

Backend (`rust/crates/host`):
- `channel/agent_worker.rs` — the run's goal now ends with `[conversation channel: <cid>]` so
  `channel.post` can target the dock's own channel.
- `channel/payload.rs` — `RichResultPayload` gains optional `sources` (opaque `Value`, the v3
  Target list a `genui` view binds `/data/{refId}` against).
- `dashboard/pin.rs` — a pinned envelope's declared `sources[]` carry onto the cell verbatim
  (first, un-hidden); extra `tools[]` no longer fold hidden duplicates for tools a declared
  target already covers.
- `agent/personas/personas.toml` — data-analyst: + `channel.post`, `dashboard.pin`,
  `dashboard.*`, `panel.*`, `layout.get/set`, `template.*`; grounding = datasources/query/
  store-read/channel-widgets/genui-widget/dashboard-widgets (ingest-series unpinned to hold the
  context budget; the tool + catalog entry remain). Identity: preview-first, save only on ask.
  widget-builder identity gains the same preview-first rule (it inherits the new tools/skill via
  `extends`).
- `workspaces/default_skills.rs` — `core.channel-widgets` added to the default grant set.

Skill:
- `docs/skills/channel-widgets/SKILL.md` (→ seeded `core.channel-widgets`): prove the query →
  `channel.post {cid, id, ts, body}` with the `rich_result` envelope → offer `dashboard.pin`.
  Includes the `view:"genui"` section (typed IR emitted directly, `$bind` pointers,
  `surface.surfaceId`, catalog names) and the explicit "a preview never touches dashboard.save".

UI:
- `lib/channel/payload.types.ts` — `sources?: Target[]` on the envelope (1:1 Rust mirror).
- `features/channel/ResponseView.tsx` — `buildCell` places declared `sources[]` first (un-hidden)
  and skips duplicate hidden leash entries (mirrors `pin.rs`).
- `features/dashboard/views/genui/genuiData.ts` — `genuiTargets`: hidden-only `sources[]` (a
  rich_result cell's leash extras) no longer shadow the real v2 `source` (falls through to
  refId `A`).

## Tests (green)

- `cargo test -p lb-host --test channel_agent_worker_test` — 10 passed, incl. NEW
  `a_run_can_post_a_rich_result_widget_into_its_own_channel` (happy path + goal-carries-cid +
  workspace isolation) and `without_channel_post_cap_the_widget_post_is_denied_but_the_run_answers`
  (capability deny).
- `cargo test -p lb-host --test widget_pin_test` — 11 passed, incl. NEW
  `pin_carries_a_genui_envelopes_declared_sources_through`.
- Persona/skill/dashboard suites re-run green: agent_persona(+coding/session), core_skills(+mcp),
  dashboard, dashboard_genui. `cargo build -p node` ok (corpus embeds the new skill).
- UI: `genuiData.test.ts` (10, incl. hidden-only-shadow regression), `ResponseView.test.tsx`
  (6, incl. genui sources[] buildCell), `tsc --noEmit` clean.

## Notes

- The first slice of this feature (same day, earlier): data-analyst got `channel.post` +
  `dashboard.pin` + the skill; live run under widget-builder successfully posted a plain `table`
  rich_result — proving the render path — but then chased `dashboard.save` for the GenUI ask,
  which motivated the preview-first identity text and the genui envelope support above.
- Restart the dev node (`make kill && make dev`) to pick up the new personas/skill/worker.
- Follow-ups recorded in the scope doc (dock `installed` threading, channel-side IR validation,
  transcript export rendering).
