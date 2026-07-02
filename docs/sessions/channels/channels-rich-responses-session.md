# Session — channel rich responses (the descriptor-driven, backend-driven render contract)

Topic: `channels`. Scope: [`channels-rich-responses-scope.md`](../../scope/channels/channels-rich-responses-scope.md).
Built alongside its first tenant, [reminders](../reminders/reminders-rich-responses-session.md).

## What shipped

The channel became a **second mount surface for the shipped dashboard widget contract** — a command,
tool, or agent answers with a `render` block in its channel `Item` body, and the channel mounts it
through the **shipped** `WidgetView`/`views/*`, leashed to the viewer's grant. No new render system,
no new trust router, no new bridge. The new code is thin: a render envelope type, a channel-side
adapter, an **open** widget registry, and the descriptor `result` field that carries the render.

**The headline correction (mid-session).** The first pass leaked *tool-specific knowledge into the
frontend* — the palette reshaped `reminder.create`'s args and hardcoded `reminder.list`'s response
render (`reminderArgs.ts` + `tool.name === "reminder.*"` branches). That contradicts the whole scope:
the channel is a **generic** front-end for the MCP tool surface (rule 7). We corrected to a **100%
backend-driven** contract:

- **The frontend names exactly one tool: `tools.catalog`.** For every command it lists it, renders its
  `input_schema` widgets by string (`x-lb.widget`), and posts its **declared** response render — never
  a `if tool.name === …` branch.
- **The descriptor carries both sides.** `ToolDescriptor` gained an optional **`result`** field (the
  `x-lb-render` envelope: the v2 rich-result `{v,view,source?,options?,action?,tools?}`). `input_schema`
  drives the *form*; `result` drives the *response*. Both are standard-JSON-Schema-compatible vendor
  extensions (`x-`/vendor keys are ignored by off-the-shelf validators — the schema stays valid).
- **Verbs accept the flat form.** A command's `input_schema` is the form the UI renders; the *verb*
  accepts those flat fields and does any shaping server-side. The UI posts collected fields verbatim.
- **The widget/view vocabulary is OPEN: UI built-ins ∪ extension-contributed widgets.** The arg-widget
  registry and the response views both resolve a string to a renderer — a built-in
  (`cron`/`select`/`table`/`chart`/`switch`/…) **or** an `ext:<id>/<widget>` (the shipped
  `WidgetView`/`ExtWidget` federation, install-gated, leashed by `[[widget]].scope ∩ grant`).
  Unknown → honest text/summary fallback.

## The pieces (thin, by design)

- **`rich_result` payload** (`rust/crates/host/src/channel/payload.rs` + `ui/src/lib/channel/payload.types.ts`,
  mirrored one-to-one): a kind-tagged body `{ kind:"rich_result", v:2, view, source?, data?, options?,
  action?, tools? }` — the same additive pattern as `query_result`/`agent_result`. `v` is versioned;
  a body with no recognized kind stays chat. `tools` is the declared set the response's bridge may
  forward (host ∩ grant, re-checked per call).
- **`ToolDescriptor.result`** (`rust/crates/mcp/src/registry.rs`): the OUTPUT half of the contract, the
  `x-lb-render` envelope a command's answer mounts as. `skip_serializing_if = "Option::is_none"` — a
  command with no declared render posts nothing extra.
- **`ResponseView`** (`ui/src/features/channel/ResponseView.tsx`): the adapter — reads the render block,
  builds a v2 `Cell`, mounts it via the shipped `WidgetView` (threaded `installed` so `ext:<id>`
  response views mount for real). No renderer/bridge/trust-router lives here — `WidgetView` owns all
  three. A `v` newer than the UI understands degrades honestly at render (not parse).
- **`ResponseTable`** (`ui/src/features/channel/ResponseTable.tsx`): the one interactive-list piece —
  the shipped `TablePanel` has no per-row control column, so a `table` with `options.rowControls`
  renders through this thin wrapper that reuses the shipped `SwitchControl`/`ButtonControl` **per row**,
  passing the row object as the control's `VarScope.values`.
- **The row-object interpolation decision (locked).** A per-row control binds the row's fields by
  passing the row as `VarScope.values` to the shipped `interpolateArgs`. Because the shipped vars engine
  matches `${name}`/`[[name]]`/`$name` (not `{{name}}`), a **row field is `${id}`** and the **interaction
  value stays `{{value}}`** (the switch bool). Reuse the shipped vars engine — no new templating slot,
  no `argsTemplate` extension.
- **The open widget registry** (`ui/src/features/channel/palette/argWidgets/registry.ts` + one file per
  widget): resolves a built-in widget, an `ext:<id>/<widget>`, or falls back to `text`. New widgets:
  `cron` (wraps the shipped `CronBuilder`), static-options `select`, plus `text`/`number`/`boolean`/
  `date`. `ActiveArgWidget` renders the resolved widget; the palette drives args through the registry
  instead of the old hardcoded `isSqlArg`/`isRuntimeArg` chain.
- **The generic palette submit** (`ui/src/features/channel/palette/CommandPalette.tsx`): collect schema
  fields → **if `tool.result` present**, post `encodeRichResult({...tool.result, source:{…, args:{…,
  …collectedArgs}}})`; **else** `onCallTool(name, collectedArgs)`. Zero reminder tool-name branch.

## Fixed-vs-generative tier (unchanged, re-asserted)

The producer picks the tier by picking the view: `chart`/`stat`/`gauge`/`table`/`switch`/`slider`/
`button` render in-process (shipped, trusted, no author code); `plot`/`d3`/`template` render in the
shipped iframe sandbox. We added **no** new in-process path for generated UI. Phase-1 generative
surface is the existing `view:"template"`; A2UI/JSON-render is an explicitly-deferred additional view.

## Security (identical to a dashboard cell)

Rendering widens nothing. A response's bridge may call only `render.tools ∩ viewer-grant`, workspace
from the viewer's token, host re-checked per call. The producer (incl. an AI) is untrusted. The deny
test bites a real ungranted **host** call, not a UI hide.

## Deliberately kept as-is (named follow-up)

The legacy `agent.invoke`/`federation.query` palette branches remain hardcoded this pass (they are
shipped and green); converting them to descriptor-declared routes like the generic `result` path is a
named follow-up in the scope doc. The blast radius on shipped code stayed minimal while the new path is
fully generic.

## Tests

Real backends throughout (rule 9). Rust: `rich_result` round-trips + stays additive
(`channel/payload.rs`); `ToolDescriptor.result` serializes/omits correctly. UI unit: the widget
registry resolves each built-in + `ext:<id>/<widget>` + unknown→text; `CronArg` round-trips; the
generic dispatch posts `descriptor.result` for any command (a `things.*` fixture, not reminders —
proving tool-agnosticism); `ResponseView` mounts each view + degrades a newer `v`. Real-gateway e2e +
the `query_result → rich_result` migration/no-regression are covered in the reminders session (the
first tenant).
