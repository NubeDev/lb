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

## 2026-07-06 (later) — the one remaining bug: wrong IR dialect, ungated at post

The live dock run posted a genui rich_result whose IR was the wrong dialect (`type` instead of
`component`, no per-component `id`, no `ir.v`, no `surface`) and NOTHING validated it at
`channel.post` — the dock rendered the invalid/draft state. Full write-up:
`docs/debugging/agent/genui-preview-posts-wrong-ir-dialect-renders-broken.md`.

Shipped:
- `channel/genui_check.rs` (NEW) — every `channel::post` (WS/HTTP/MCP funnel through it) now
  structurally validates a `kind:"rich_result"` + `view:"genui"` body, after authorize, before
  persist. Chat / non-genui payloads pass untouched; a preview with no IR is an error.
- `dashboard/genui.rs` — the cell check's core extracted as `pub check_genui_block` (ONE validator,
  both seams); tightened to mirror `@nube/genui` `validate.ts` (per-component `id` must repeat the
  map key); messages now name the fix (`component` not `type`; the `surface:{surfaceId,root}`
  shape) — on the channel path they ARE the agent-loop feedback.
- `ChannelError::BadInput` (new) → `ToolError::BadInput` over MCP (tool.rs + chart_pref_tool.rs);
  gateway `POST /channels/{cid}/messages` maps it to 400 (was blanket 403).
- `docs/skills/channel-widgets/SKILL.md` — "Common IR mistakes" block (component/id/v/surface,
  decorative controls, repost with the same id).

Tests (green): `genui_check` units 5/5; `channel_agent_worker_test` 11/11 incl. NEW
`a_malformed_genui_rich_result_is_rejected_and_the_corrected_repost_lands` (wrong dialect rejected
and fed back, corrected repost is the only genui item that lands, run still answers);
`dashboard_genui_test` 8/8, `dashboard_test` 10/10, `widget_pin_test` 11/11, messaging suites,
`lb-role-gateway` full suite (native_call needs `cargo build -p echo-sidecar` first).

Live retest checklist: RESTART THE NODE (`make kill && make dev` — never hot-reloads), then in the
dock: "use this query to build a genui widget … add a limit slider and run button" → expect a
rendered card (a bad first attempt should be rejected and self-corrected within the run), then Pin
and verify the dashboard cell.

## 2026-07-06 (live retest round 2) — string-IR stall fixed + Playwright e2e

Live dock retest after the gate shipped: the gate WORKED (every bad IR rejected, one post landed)
but the loop stalled on a new shape — the model sent `options.genui.ir` as a JSON-ENCODED STRING,
and "must be an object" wasn't feedback enough (three identical retries). The serde-strictness-
at-an-AI-seam tax again.

Shipped:
- `dashboard/genui.rs::normalize_genui_block` (lenient-args): a string `ir` that parses to a JSON
  object is rewritten to the object — applied at BOTH seams (`dashboard_save` pre-validation;
  `channel::genui_check`, which returns the normalized body so the renderer sees the real IR).
  The remaining non-object error now names the fix ("not a string — pass the IR inline, unquoted").
- Skill "Common IR mistakes" gained the object-not-string line.
- NEW Playwright e2e `ui/e2e/channel-genui-preview.spec.ts` (3/3 green vs the REAL rebuilt node on
  :8080 + built shell on :4173): (1) wrong-dialect post rejected with the fix named over the real
  `/mcp/call` bridge; (2) string-ir post lands and history carries the parsed OBJECT ir; (3) the
  browser renders the composed genui surface in the channel — title text, limit slider, Run button,
  and the table live-bound to `federation.query demo-buildings` (screenshot
  `ui/e2e/__screenshots__/channel-genui-preview.png`) with the Pin affordance.

Tests: `genui_check` units 7/7, `dashboard_genui_test` 8/8 (new stringified-ir-saves case),
`channel_agent_worker_test` 11/11, `cargo build -p node` ✓, e2e 3/3.

## 2026-07-06 (live retest round 3) — one-turn feedback instead of five

Live retest ×2: run 2 CONVERGED (the widget posted and rendered, valid IR, honest answer) — but it
took 5 rejected posts because the validator returned the FIRST defect only, teaching one field per
turn; run 1 burned its whole budget on queries and never posted. Fix: `check_genui_block` now
collects EVERY independent defect into one message ("4 defects — fix ALL in one retry: …") and
appends a minimal complete valid IR template to every structural rejection, so a from-scratch
rewrite lands in one retry. Verified live over `/mcp/call`: the round-2 first post now gets all
four defects + the template in a single tool error. Suites green (genui units 7/7,
dashboard_genui 8/8, channel_agent_worker 11/11); e2e 3/3 against the rebuilt node.

## 2026-07-07 (live retest round 4) — channel.post had NO arg schema

Live run burned its whole 16-turn budget on 13 × `missing arg: cid`: `channel.post` was a
name-only catalog row (no `ToolDescriptor`), so the model saw an argument-less tool and guessed
arg names — the `dashboard.save`/`tool-schema-dropped` lesson replayed on the one verb this whole
feature hangs on. Shipped:
- `channel/tool.rs::post_descriptor` — real `{cid, id, ts, body}` schema, registered in
  `tools/descriptor.rs::host_descriptors`; `cid`'s description says it comes from the goal's
  `[conversation channel: <cid>]` line, `body` points at the channel-widgets skill.
- `tools/descriptor.rs::validate_args` — a missing-required-arg error now appends the arg's own
  `x-lb.description` (benefits every schema'd verb); handler-side cid/id misses steer likewise,
  and `channel` is accepted as a cid alias.
Live-verified on the restarted node: `tools.catalog` serves the schema; a cid-less post returns
"missing required arg: cid — The channel to post into. In an agent run, use … `[conversation
channel: <cid>]`". Suites: lb-host lib 147, channel_agent_worker 11, persona_menu_full_catalog 2 ✓.

## 2026-07-07 (round 4b) — the DOCK surface proven directly

The user tests in the AGENT DOCK, not Channels — so the render proof was moved to the right
surface: NEW `ui/e2e/agent-dock-genui-preview.spec.ts` seeds a real dock session channel
(`dock-user-ada-…`, the dockId grammar) over the real gateway with the exact `channel.post` a run
makes, opens the dock in Chromium, selects the session, and asserts the composed genui surface
renders inside the dock panel (screenshot `agent-dock-genui-preview.png`: title + live
federation-bound table + Pin). 1/1 green. Conclusion: the dock render path works; every "no
preview" the user saw was the POST never landing (wrong dialect → string ir → missing cid), each
now closed host-side.

## 2026-07-07 (round 5) — missing brace landed as chat, rendered raw in the dock

Live run: one-retry convergence ✓, post ✓ — but raw JSON in the dock. The body was missing ONE
closing brace; the gate's chat-tolerance passed it through as text. Fix in `genui_check`: an
attempted payload (`{`-leading, names `"kind"`) with broken JSON is rejected with the parser's
position; chat unaffected. Unit: `an_attempted_payload_with_broken_json_is_rejected_not_landed_as_chat`.
Live-verified with the exact stored body (403 + position) and its brace-fixed twin (lands).
Suites: genui units 8/8, channel_agent_worker 11/11, messaging 3/3; node rebuilt + restarted.
