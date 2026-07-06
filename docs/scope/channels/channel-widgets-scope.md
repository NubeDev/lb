# Channel-widgets scope — the agent answers with a LIVE widget in the conversation

Status: **SHIPPED v1** (2026-07-06) — see `sessions/agent/channel-widgets-session.md`.

The ask (user, 2026-07-06): in the agent dock, ask a data question and get a rendered **widget
preview** — a table/chart/stat, and a composed **GenUI/OpenUI** layout — bound to the REAL query,
with the option to save it as a dashboard widget/panel afterwards. Previewing must not write a
dashboard: the live failure mode was the widget-builder persona driving `dashboard.save` for what
was meant to be a look.

## Decisions (v1)

1. **The agent posts a `rich_result`; nothing new renders.** The dock already renders channel
   items through `MessageList → MessageItem → ResponseView → WidgetView` with live data via
   `usePanelData`, and `PinToDashboard` already offers the save. So the whole feature is: teach
   the agent to `channel.post` the shipped render envelope into its own conversation channel.
   Rejected: a `render` block inside `agent_result` (a second render path in the answer card) and
   any dock-only renderer (rule 9 fork).
2. **The run learns its channel id from the goal.** `agent_worker` appends
   `[conversation channel: <cid>]` — a fact, not an instruction; the `core.channel-widgets`
   skill owns the choreography (prove the query → post the envelope → offer the pin).
3. **GenUI previews ride the same envelope.** `view:"genui"` + `options.genui.{v,ir}` (the typed
   IR, emitted directly — no Lang round-trip headless) + NEW envelope `sources[]` (the v3 Target
   shape) for multi-ref `/data/{refId}` bindings. `ResponseView.buildCell` and `dashboard.pin`
   both carry declared `sources[]` through verbatim (mirrored implementations), and hidden
   leash-extras no longer shadow a v2 `source` in `genuiTargets`.
4. **Preview ≠ save.** Both the data-analyst and widget-builder identities now state: a posted
   widget IS the preview; `dashboard.save`/`dashboard.pin` only when the user asks to keep it.
5. **Personas converge (user decision).** The data-analyst absorbs the widget-authoring surface
   (`dashboard.*`, `panel.*`, `layout.*`, `template.*`) + the genui/widget grounding skills —
   data and dashboards are one job; no persona-switching. The wall (`persona ∩ agent ∩ caller`)
   is unchanged; this is advertisement, not authorization.

## Follow-ups (named, not started)

- Dock `MessageList` gets `installed` threaded so `ext:<id>/<widget>` response views mount in the
  dock (core views render today).
- Channel-side structural validation of a posted `rich_result` genui IR (today: view-time
  validate/placeholder only; `dashboard.pin`/`dashboard.save` still validate at persist).
- `exportTranscript` renders a `rich_result` as a labeled widget block instead of raw JSON.

## Related

- `channels-rich-responses-scope.md` (the envelope + render path), `genui/genui-scope.md` (the IR
  + catalog), `frontend/dashboard/widget-builder-scope.md` (`dashboard.pin`, Slice B).
