# Render Template Widget (the sandboxed-iframe scripted view)

**Status:** shipped (part of the widget-builder scope, "Scripted views").
**Trust tier:** untrusted author code → **opaque-origin sandboxed iframe** (now `plot`/`d3` only).

> **The eval-free `template` engine was promoted IN-PROCESS** (2026-07-05, render-template-inprocess
> scope) — it runs NO author JavaScript (pure `{{path}}`/`{{#each}}` interpolation + a sanitized
> `innerHTML`), so the iframe sandbox bought nothing for it and cost a second document + a per-tick
> `postMessage` tax. It now renders as a first-class `TemplateView` (sibling of `GenUiView`) over the
> same `usePanelData` rows + the same leashed bridge; the sandbox is replaced by **DOMPurify**
> (`sanitizeTemplateHtml.ts`) + the existing `dashboard.save`/`template.save` cap as the authoring trust
> gate. See [`render-template-inprocess-scope.md`](render-template-inprocess-scope.md) (the scope) +
> [`../../../sessions/frontend/dashboard/render-template-inprocess-session.md`](../../../sessions/frontend/dashboard/render-template-inprocess-session.md)
> (the shipped session). **This doc remains the reference for the `plot`/`d3` engines**, which keep the
> sandboxed-iframe tier (their snippets `eval` via `new Function` — real RCE; the sandbox is load-
> bearing). The `engine` type on `WidgetIframe`/`ScriptedView`/`buildIframeSrcdoc` is now narrowed to
> `"plot" | "d3"` so a future caller cannot route `template` here (the class of bug is unrepresentable).

This is the widget you remembered as "a render template widget that's an iframe."
There is **no single `RenderTemplate` component** — it's a small stack. The dashboard
cell picks `view: "plot"` or `view: "d3"`, and that mounts a scripted view whose author-written
JS runs inside a sandboxed `<iframe srcdoc>`, talking to the host only through a `postMessage`
bridge. (A `view:"template"` cell used to mount here too; it now mounts `TemplateView` in-process.)

---

## At a glance

| Concern | Where |
|---|---|
| Registered as | `view: "plot"` / `view: "d3"` in [WidgetView.tsx](../../../../ui/src/features/dashboard/views/WidgetView.tsx) (the eval-free `view:"template"` now routes to [TemplateView](../../../../ui/src/features/dashboard/views/TemplateView.tsx) in-process) |
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

## The `template` engine — now in-process

> The eval-free `template` engine USED to share this iframe stack. It was promoted IN-PROCESS
> (2026-07-05) because it runs NO author JavaScript — only pure `{{path}}`/`{{#each}}` interpolation
> (the eval-free [templateInterpolate.ts](../../../../ui/src/features/dashboard/builder/templateInterpolate.ts))
> over the panel's source rows + `innerHTML`, with `[data-call]` buttons routed through the leashed
> bridge. The iframe sandbox bought nothing for it and cost a second document + a per-tick postMessage
> tax + a broken embedded-frame feel. It now renders as **`TemplateView`**
> ([TemplateView.tsx](../../../../ui/src/features/dashboard/views/TemplateView.tsx)) — a sibling of
> `GenUiView` — over the SAME `usePanelData` rows + the SAME leashed, host-re-checked bridge. The one
> new guard replacing the sandbox is a markup sanitizer — **DOMPurify** wrapped in one file
> ([sanitizeTemplateHtml.ts](../../../../ui/src/features/dashboard/builder/sanitizeTemplateHtml.ts))
> — plus the existing `dashboard.save`/`template.save` cap as the authoring trust gate (the same trust
> class as genui). The `[data-call]` wiring lives in
> [wireTemplateDataCalls.ts](../../../../ui/src/features/dashboard/builder/wireTemplateDataCalls.ts)
> (Decision 5: reads ONLY `data-call`/`data-args`, never an author inline handler). See
> [`render-template-inprocess-scope.md`](render-template-inprocess-scope.md) +
> the shipped session for the full security model (the XSS-vector suite is the new definition of done).

The template **contract** (data binding + `data-call` writes) is unchanged by the tier move — an author
who wrote `<ul>{{#each rows}}<li>{{seq}}</li>{{/each}}</ul>` + a `<button data-call="…" data-args='…'>`
sees identical behavior, just rendered in the shell document (theme/fonts, no frame flicker) instead of
an opaque-origin iframe.

`plot`/`d3` (below) STAY on the iframe tier — their snippets `eval` via `new Function`, which is real
RCE; the sandbox is load-bearing for them.

---

## The `plot`/`d3` engines (author JS, sandboxed)

Unlike the (now in-process) `template` engine, `plot`/`d3` **run author code** via
`new Function("bridge","el","engine","rows", cfg.code)`. The author may read rows straight from the
`rows` arg (the parent's `usePanelData`) OR fetch via `bridge.call`; a mounted element replaces the
`#root`. The sandbox + grant + host re-check are the three guards (widget-builder scope, "Scripted
views ... may write").

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
