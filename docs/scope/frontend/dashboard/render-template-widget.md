# Render Template Widget (the sandboxed-iframe scripted view)

**Status:** shipped (part of the widget-builder scope, "Scripted views").
**Trust tier:** untrusted author code → **opaque-origin sandboxed iframe**.

This is the widget you remembered as "a render template widget that's an iframe."
There is **no single `RenderTemplate` component** — it's a small stack. The dashboard
cell picks `view: "template"`, and that mounts a scripted view whose author-written
HTML/JSX runs inside a sandboxed `<iframe srcdoc>`, talking to the host only through a
`postMessage` bridge. The same stack also powers the `plot` and `d3` engines; `template`
is the eval-free HTML variant.

---

## At a glance

| Concern | Where |
|---|---|
| Registered as | `view: "template"` in [WidgetView.tsx](../../../../ui/src/features/dashboard/views/WidgetView.tsx#L119-L120) (alongside `plot`, `d3`) |
| React view (mounts it) | [ScriptedView.tsx](../../../../ui/src/features/dashboard/views/ScriptedView.tsx) |
| Iframe host + bridge parent | [WidgetIframe.tsx](../../../../ui/src/features/dashboard/builder/WidgetIframe.tsx) |
| `srcdoc` + in-frame bootstrap | [iframeRuntime.ts](../../../../ui/src/features/dashboard/builder/iframeRuntime.ts) |
| `{{path}}`/`{{#each}}` interpolator | [templateInterpolate.ts](../../../../ui/src/features/dashboard/builder/templateInterpolate.ts) (+ `.test.ts`) |
| Host-mediated call/watch bridge | [widgetBridge.ts](../../../../ui/src/features/dashboard/builder/widgetBridge.ts) |
| Builder editor (Inline/Saved) | [TemplateSourceField.tsx](../../../../ui/src/features/dashboard/builder/editors/TemplateSourceField.tsx) |
| Durable template API (client) | [template.api.ts](../../../../ui/src/lib/dashboard/template.api.ts) |
| Durable `render_template` record | [render_templates/model.rs](../../../../rust/crates/host/src/render_templates/model.rs) |
| MCP verbs `template.*` | [render_templates/tool.rs](../../../../rust/crates/host/src/render_templates/tool.rs) |
| Tests (real gateway) | `ui/src/features/dashboard/builder/widgetBuilder.gateway.test.tsx`, `ui/e2e/dashboard-widget.spec.ts` |

---

## The data flow

```
Dashboard cell  OR  channel rich_result   (view: "template")
  └─ ScriptedView
       ├─ usePanelData(cell)  ── the ONE data hook ──▶  rows  (source-backed, via the bridge)
       └─ WidgetIframe             PARENT side of the bridge; sandboxed <iframe>
            │  frame.srcdoc = buildIframeSrcdoc({ engine, code, tools, data })   // initial rows
            │  ── postMessage {type:"render-data", data} ─▶  (live refresh, no rebuild)
            ▼
       iframe (opaque origin, sandbox="allow-scripts")
            ├─ root.innerHTML = interpolateTemplate(code, data)   // {{path}} + {{#each}}, escaped
            ├─ [data-call] buttons → bridge.call(tool, args)      // host-mediated WRITE
            └─ ── postMessage {type:"frame-ready"} ─▶  (parent replies with freshest rows)
            │
            │   ── postMessage {type:"bridge-call", id, tool, args} ──▶  (a write button)
            ◀── postMessage {type:"bridge-reply", id, result|error} ──
       WidgetIframe re-checks tool ∈ cell.tools, forwards to →
       WidgetBridge.call → invoke("mcp_call") → HOST re-checks cap + workspace
```

The frame **cannot reach the network directly** (CSP `connect-src 'none'`). Reads arrive as
data pushed in by the parent's `usePanelData`; every write goes parent → host; the
**session token never enters the frame**.

---

## Where the template comes from (inline vs saved)

A `template` cell stores its source one of two ways
([ScriptedView.tsx:24-25](../../../../ui/src/features/dashboard/views/ScriptedView.tsx#L24-L25)):

- **Inline** — `cell.options.code` (a small HTML/JSX string, **≤ 4 KB** = `INLINE_MAX_BYTES`).
- **Saved** — `cell.options.templateId` referencing a durable `render_template:{id}` row,
  fetched via `getTemplate(templateId)` → the `template.get` MCP verb. Body **≤ 64 KB**
  (`TEMPLATE_MAX_BYTES`).

The builder's [TemplateSourceField](../../../../ui/src/features/dashboard/builder/editors/TemplateSourceField.tsx)
is the Inline/Saved toggle. Saved templates are listed from `template.list` (never REST).

The durable record ([model.rs](../../../../rust/crates/host/src/render_templates/model.rs)):

```rust
pub struct RenderTemplate {
    pub id: String,        // render_template:{id}, unique per workspace
    pub title: String,
    pub engine: Engine,    // Template | Plot | D3  (serde lowercase)
    pub code: String,      // ≤ TEMPLATE_MAX_BYTES (64 KB)
    pub author: String,    // only the author may update/delete
    pub updated_ts: u64,
    pub deleted: bool,     // soft-delete tombstone
}
```

Code is **state**, so it lives in SurrealDB (rules 2/4), not `localStorage`. The host
**never executes** the code — it only stores and serves it; rendering happens client-side
in the iframe tier.

---

## The `template` engine (eval-free, data-driven)

Unlike `plot`/`d3` (which run author code via `new Function(...)`), the `template` engine
does **no eval**. It binds the panel's **source rows** into the author markup via a small,
pure, unit-tested interpolator ([templateInterpolate.ts](../../../../ui/src/features/dashboard/builder/templateInterpolate.ts)),
then wires `[data-call]` write buttons ([iframeRuntime.ts](../../../../ui/src/features/dashboard/builder/iframeRuntime.ts)):

```js
if (cfg.engine === "template") {
  window.__bridge = bridge;
  root.innerHTML = interpolateTemplate(cfg.code, cfg.data); // {{path}} + {{#each}}, data escaped
  root.querySelectorAll("[data-call]").forEach((el) => {
    el.addEventListener("click", async () => {
      const tool = el.getAttribute("data-call");
      const args = JSON.parse(el.getAttribute("data-args") || "{}");
      await bridge.call(tool, args);                          // host-mediated write
      el.setAttribute("data-called", "ok");
    });
  });
}
```

So the template contract has three parts:

1. **Data binding** — `{{path}}` scalar interpolation + `{{#each list}}…{{/each}}` iteration
   over the panel's rows. The data context is the `SourceState`:
   `{ rows, latest, loading, denied }`. Inside an `each` block the context **is the item**,
   so `{{field}}` reads the row and `{{.}}` is the whole row. **Every interpolated data value
   is HTML-escaped** — a row value can't inject markup (the template *markup* is author-owned
   and trusted for structure; the *data* is the viewer's grant and is escaped).
2. **Markup** is set as `innerHTML` (the CSP + opaque origin bound what it can do).
3. **Write buttons** are declared with attributes:
   ```html
   <button data-call="store.query" data-args='{"sql":"SELECT seq FROM series LIMIT 1"}'>
     Refresh
   </button>
   ```
   On click, the runtime routes `data-call`/`data-args` through the bridge and stamps
   `data-called="ok" | "err"` for feedback.

The default inline snippet
([TemplateSourceField.tsx](../../../../ui/src/features/dashboard/builder/editors/TemplateSourceField.tsx)):

```html
<div class="p-2 text-xs">
  <div class="text-muted">{{rows.length}} rows</div>
  <ul>
    {{#each rows}}<li>{{seq}}</li>{{/each}}
  </ul>
  <button data-call="store.query" data-args='{"sql":"SELECT seq FROM series LIMIT 1"}'>Refresh</button>
</div>
```

### Where the rows come from (the one data hook)

The template does **not** fetch its own data. `ScriptedView` loads the cell's source rows
through **`usePanelData`** — the *same* hook every read view (chart/stat/gauge/table) uses —
and passes them to the frame. This is the whole point of the design: a template is
data-driven **identically on a dashboard and in a channel response**, because both build a
`Cell` with a `source`, and both flow through `usePanelData → ScriptedView → WidgetIframe`.
It also inherits the Phase-3 `viz.query` backend transform pipeline and auto-refresh
(`refreshKey`) for free.

### Live updates without rebuilding the frame

Rows reach the sandbox as an **initial `srcdoc` config value** and then as live
`render-data` `postMessage`s. On first paint the frame posts `{type:"frame-ready"}` and the
parent replies with the freshest rows (covering the race where data resolved after the
`srcdoc` was built); thereafter every refresh re-posts changed rows and the frame
re-renders — **the iframe is never rebuilt on a data change** (no CDN re-fetch or flicker
for `plot`/`d3`; a `template` just re-runs the cheap `innerHTML`). `plot`/`d3` snippets also
receive the rows as a 4th `rows` argument, so they can render pre-fetched data without a
bridge round-trip.

> **Limitations.** `{{#each}}` is **single-level** (the first `{{/each}}` closes the block —
> no nested `each`). There are no conditionals (`{{#if}}`) yet. An object value interpolated
> with `{{path}}` renders as compact JSON (honest, not `[object Object]`). Unknown paths
> render empty, never crash.

---

## Security model (why it's an iframe at all)

The whole point of this tier is running **untrusted author code** without giving it the
session or the workspace.

**Sandbox.** `sandbox="allow-scripts"` and deliberately **no `allow-same-origin`**
([WidgetIframe.tsx:86](../../../../ui/src/features/dashboard/builder/WidgetIframe.tsx#L86)).
The frame runs in a **unique opaque origin**: it cannot read the parent's cookies,
`localStorage`, or the session token, and its `postMessage` origin is the string `"null"`.

**CSP** (baked into the `srcdoc`,
[iframeRuntime.ts:39-40](../../../../ui/src/features/dashboard/builder/iframeRuntime.ts#L39-L40)):

```
default-src 'none';
script-src 'unsafe-inline' https://cdn.jsdelivr.net;   /* our bootstrap + pinned plot/d3 CDN */
style-src 'unsafe-inline';
img-src data:;
connect-src 'none';                                     /* NO direct network — bridge only */
```

**Content injection is `srcdoc`, never `src`.** The `{engine, code, tools}` config is
JSON-serialized with `<` escaped to `<` (script-injection guard) and embedded as a
`<script type="application/json">` block the bootstrap reads via `JSON.parse`
([iframeRuntime.ts:26-30, 50, 100](../../../../ui/src/features/dashboard/builder/iframeRuntime.ts#L26-L30)).

**postMessage protocol** (bidirectional):

| Direction | Message |
|---|---|
| frame → parent | `{type:"bridge-call", id, tool, args}` |
| frame → parent | `{type:"bridge-watch", id, tool, args}` / `{type:"bridge-unwatch", id}` |
| frame → parent | `{type:"rendered"}` / `{type:"render-error", error}` |
| parent → frame | `{type:"bridge-reply", id, result \| error}` |
| parent → frame | `{type:"watch-event", id, event}` |

**Trust discipline (source-identity, because origin is `"null"`):**

- Parent accepts a message only if `e.source === frame.contentWindow`
  ([WidgetIframe.tsx:38](../../../../ui/src/features/dashboard/builder/WidgetIframe.tsx#L38)).
- Frame accepts a message only if `e.source === window.parent`
  ([iframeRuntime.ts:70-71](../../../../ui/src/features/dashboard/builder/iframeRuntime.ts#L70-L71)).

**Three guards on every call** (defense in depth):

1. **Frame → parent:** `WidgetIframe` re-checks `allowed.has(msg.tool)` (the cell's
   `tools` set) before forwarding.
2. **Bridge:** `makeWidgetBridge` re-checks `allowed.has(tool)` and rejects
   `out_of_scope: <tool>` locally.
3. **Host:** `mcp_call` re-checks the capability **and** the workspace server-side (from
   the token the shell holds — never from the cell or the frame).

A widget bypassing guards 1–2 still hits a server-side deny at guard 3.

---

## Live data (watch)

Beyond one-shot `call`, the bridge supports streaming
([widgetBridge.ts:52-67](../../../../ui/src/features/dashboard/builder/widgetBridge.ts#L52-L67)):
`bridge.watch(tool, args, onEvent) => unsubscribe`, limited to `WATCH_VERBS =
{"series.watch", "bus.watch"}`. These map onto the shipped **series SSE**
(`GET /series/{s}/stream`) and bus stream — no new transport, no polling. The token is
attached to the `EventSource` URL **server-side**, so it never appears in any
widget-visible payload. Streams tear down on unmount (`WidgetIframe` invokes the returned
unsubscribe for every live watcher).

---

## MCP surface (`template.*`)

CRUD over the durable record, each gated by a capability
([tool.rs](../../../../rust/crates/host/src/render_templates/tool.rs),
[template.api.ts](../../../../ui/src/lib/dashboard/template.api.ts)):

| Verb | Capability | Notes |
|---|---|---|
| `template.save` | `mcp:template.save:call` | create/update; author-only on update |
| `template.get` | `mcp:template.get:call` | returns the full `RenderTemplate` (with `code`) |
| `template.list` | `mcp:template.list:call` | returns `RenderTemplateSummary[]` (no code body) |
| `template.delete` | `mcp:template.delete:call` | author-only; soft-delete tombstone |

All are workspace-scoped (the hard wall, rule 6) and author-owned for mutation.

---

## Using it in a channel (rich response)

The template widget is **not** dashboard-only. Channel rich responses render through
[ResponseView.tsx](../../../../ui/src/features/channel/ResponseView.tsx) → the *same*
`WidgetView` dispatcher, so a response with `view: "template"` mounts the identical
`ScriptedView → WidgetIframe` stack — no channel-specific code. `ResponseView.buildCell`
folds `source` + `tools` into the cell so the bridge leash = `render.tools ∩ grant`.

To emit a template widget from a tool/agent, return a **v2 render envelope**:

```jsonc
{
  "v": 2,
  "view": "template",
  "source": { "tool": "reminder.list", "args": {} },   // the read that feeds `rows`
  "tools": ["reminder.complete"],                        // any WRITE verbs data-call buttons hit
  "options": {
    "code": "<ul>{{#each rows}}<li>{{title}} <button data-call=\"reminder.complete\" data-args='{\"id\":\"{{id}}\"}'>done</button></li>{{/each}}</ul>"
    // OR, for a larger/shared body: "templateId": "render_template:my-reminders"
  }
}
```

Two rules carried over from the dashboard case:

- **Must be source-backed.** A template renders the rows from its `source`. An envelope with
  inline `data` and no `source` has no shipped read path and degrades to an honest note
  (`ResponseView` header, "Data path"). Name a `source`.
- **Writes are leashed.** A `data-call` verb must be in the envelope's `tools`; the host
  re-checks `render.tools ∩ grant` server-side, so a viewer without the cap is denied there
  regardless of what the response declared.

## Tests

The interpolator is a **pure function**, unit-tested directly
([templateInterpolate.test.ts](../../../../ui/src/features/dashboard/builder/templateInterpolate.test.ts)):
scalar binding, `{{#each}}` iteration, **data-value escaping** (no markup injection from a
row), object→JSON, unknown-path→empty. This is the *same* function the frame runs (embedded
via `.toString()`), so the test covers the real interpreter, not a copy.

Per CLAUDE §9 (no mocks), the integration coverage runs against a **real in-process gateway**
(`widgetBuilder.gateway.test.tsx`):

- `render_template` CRUD round-trip (`save → get → list → delete`).
- Per-verb **capability deny**.
- **Workspace isolation** (a template in workspace A is invisible in workspace B).
- **Server-side write deny even when the bridge filter is bypassed** (proves guard 3).
- Bridge `out_of_scope` rejection (guard 2).

E2e smoke: `ui/e2e/dashboard-widget.spec.ts`. Backend verb tests live beside each verb
in `rust/crates/host/src/render_templates/*.rs`.

---

## Related docs

- `docs/public/frontend/dashboard.md` — the scripted-view / iframe-tier summary (trust
  tier, CSP, postMessage) and shipped status.
- `docs/scope/frontend/dashboard/widget-builder-scope.md` — the originating scope
  ("Scripted views", "No in-process untrusted code", open Q3 on the inline threshold).
- `docs/debugging/frontend/ext-widget-iframe-tier-cannot-resolve-bare-react.md` — why the
  iframe tier is restricted to scripted author code (installed ext widgets federate
  in-process instead).
- Sessions: `docs/sessions/frontend/widget-builder-session.md`,
  `docs/sessions/frontend/widget-builder-followups-session.md`.
