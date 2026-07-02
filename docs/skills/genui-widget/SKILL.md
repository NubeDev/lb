---
name: genui-widget
description: >-
  Author a dashboard "genui" widget: design a live, data-bound widget from a natural-language
  request by emitting OpenUI Lang (shell-streamed) or the typed GenUI IR directly (headless),
  discovering and binding real data (flows / store / series) as ordinary v3 sources[], and
  accepting once (parse → normalize → validate → size-check) into a `dashboard.save` cell.
---

# genui-widget — author AI-driven dashboard widgets

You are being asked to design a **dashboard widget** whose layout you author and whose data flows
through the platform's already-shipped bindings. You design the widget ONCE; it then renders live
with no further model calls (`@nube/genui`: agent authors durable IR *spec*; live *data* flows
through gated `sources[]`). Never put yourself in the render/refresh hot path.

## When to use / the two choreographies

You are invoked to author a `view:"genui"` dashboard cell. There are exactly two ways in, and you pick
by how you were reached:

**(a) Shell-streamed Lang** — the dashboard "AI widget" builder called `agent.invoke` under the
**caller's** principal and opened the run stream (`agent.watch`). Emit **OpenUI Lang** progressively:
one `id = Component(args)` statement per line, so partial output renders as it streams. The entry
point is the statement literally named `root` (e.g. `root = Stack("vertical", [a, b])`). The shell
re-parses each `text-delta` into the IR and fills the live preview. You do **not** validate or persist
— on **accept** the shell runs parse → normalize → validate → size-check **once**, then writes the
typed IR via `dashboard.save`. Use this when a human is watching a preview.

**(b) Headless direct-IR** — any MCP caller reached you without a streaming preview: the gateway's
`POST /mcp/call` (CLI, API-key machine principal), routed MCP over Zenoh, or a third-party agent on
the `external-agent` ACP runtime. Skip Lang entirely and emit the **typed IR directly** in
`options.genui = { v, ir, meta? }`, written straight through `dashboard.save`. The host structurally
validates it on save (IR `v` known, whole block ≤ ~8 KB, every `component` name resolves in the
catalog) and gives you the **same loud rejection** the shell gives — a malformed cell is refused at
write time, never degraded at view time. Use this when there is no human and no preview.

Both paths persist the identical artifact: a typed `IrSpec` plus the `sources[]` you chose. You author
the widget **once**; it then renders live off gated `sources[]` with no further model call.

## Data discovery & binding (the core skill)

This is the job. A widget with no real data is worthless; a widget that leaks a table dump into the
model provider is a bug. The choreography:

1. **Enumerate candidates** — `flows.list` / `flows.nodes` to find flow nodes, `series.find` for
   series, `store.schema` for stored tables. Descriptors and schema first, always.
2. **Sample, bounded** — only then read values: `flows.node_state`, `series.read`, `store.query`. Cap
   it at **≤20 rows per candidate source** — NEVER a table dump. This sampling is the moment workspace
   data reaches the configured model provider (Decision 4); keep it small. Your data reach is
   `caller ∩ agent` — you can never sample or bind a source the invoking user couldn't read.
3. **Bind `sources[]`** — add one v3 `Target` `{ refId, tool, args }` per source, exactly as the
   dashboard source-picker would. Assign short refIds (`A`, `B`, …). For `flows.node_state`, the flow
   read tool takes only the flow `id` as its real arg; the node id and port name are stashed under the
   `__flowNode` / `__flowPort` arg convention (default port `"payload"`).
4. **Emit JSON-Pointer bindings** — in the IR, bind a component's data prop to `/data/{refId}/…` as a
   `{ "$bind": "/data/…" }` object. A flow node_state target `A` → `{"$bind":"/data/A/value"}`; a
   series target `B` → its rows at `{"$bind":"/data/B/rows"}`. At view time `usePanelData` resolves
   each target and patches the surface data model at `/data/{refId}`; the renderer re-binds.

**Worked example** (the scope's Example flow — a flow counter next to a 24 h series chart, red when the
counter stalls):

```json
"sources": [
  { "refId": "A", "tool": "flows.node_state",
    "args": { "id": "flow_demo", "__flowNode": "counter_1", "__flowPort": "payload" } },
  { "refId": "B", "tool": "series.watch", "args": { "series": "office/temp", "range": "24h" } }
]
```

```text
count = Stat({"$bind":"/data/A/value"}, "Flow count", "", "bad")
chart = Timeseries({"$bind":"/data/B/rows"}, "ts", "value", "", "line")
root  = Grid(2, [count, chart])
```

The `tone:"bad"` on the Stat carries the "red when it stalls" rule via a threshold/tone prop; layout
components take their children as a trailing array of refs (`Grid(2, [count, chart])`).

## Actions (controls)

`button`, `slider`, and `switch` emit an action — `press`, `change`, `toggle` respectively — that the
host maps to an MCP tool call over the **leashed bridge** and re-capability-checks host-side against
the cell's tool leash (`cellTools`). You never receive a token, and you can only wire an action to a
tool the cell already declares in its leash; the action→tool binding lives on the cell, not in the
frame. A control bound to a tool outside the leash has its bridge call rejected host-side.

## Accept / rejection

parse → normalize → validate → size-check runs **once**, loudly, at accept (path a) or at
`dashboard.save` (path b). **Normalize** fixes sloppiness and surfaces each fix as a preview warning:
an unknown component becomes a labelled `placeholder`, a dangling child id is dropped, a wrong-typed
prop is coerced or defaulted. A spec that can't validate, or whose `options.genui` block exceeds
~8 KB, is **REJECTED** with a stated message. Do not fight the bound — simplify the widget ("one
widget, one job"); an oversized catalog spec is almost always a bad generation. The host re-runs the
structural checks on `dashboard.save`, so headless writers get the identical rejection.

## Capabilities

Authoring needs:
`mcp:agent.invoke:call` + `mcp:agent.watch:call` + `mcp:dashboard.save:call` + this skill grant, plus
the **read caps for every source you bind** (e.g. `mcp:series.read:call`, `mcp:flows.node_state:call`,
`mcp:store.query:call`). Your data reach is `caller ∩ agent` — you can never bind a source the invoking
user couldn't read. No `dashboard.save` grant → no "AI widget" builder entry at all. At **view time** a
source the viewer lacks the cap for renders the standard `usePanelData` denied/empty panel state — not
a crash, and nothing genui-specific.

## The catalog — the only components you may use

The renderer will ONLY instantiate components from this catalog. Anything else is turned into a
labelled placeholder at accept and warned about. Emit only these, with these props:

<!-- BEGIN GENERATED CATALOG (do not edit — `pnpm --filter @nube/genui gen:skill`) -->

```text
Components you may use (and ONLY these):

- barchart(data: binding, horizontal?: boolean)
    A bar chart over `data` ({name,value}[]); `horizontal` flips the orientation.

- button(label: string, value?: binding)
    A button; emits `press` with its value when clicked.
  actions: press

- card(title?: string, children?: array)
    A bordered card with an optional title, wrapping child components. `children` is an array of component refs, e.g. Card("Room", [a, b]).

- gauge(value: binding, min?: number, max?: number, thresholds?: array)
    A radial arc gauge showing `value` between `min`/`max` with optional thresholds.

- grid(columns?: number, children?: array)
    Responsive grid; `columns` (1..6) per row. `children` is an array of component refs, e.g. Grid(2, [a, b, c]).

- markdown(value: string)
    Safe minimal markdown (headings, bold, italic, code, links, lists) — no raw HTML.

- piechart(data: binding, pieType?: "pie" | "donut")
    A pie/donut chart over `data` ({name,value}[]).

- placeholder(label?: string)
    Inert labelled placeholder (normalize target for an unknown component).

- slider(value?: binding, min?: number, max?: number, step?: number, label?: string)
    A range slider; emits `change` with the committed value on release.
  actions: change

- stack(direction?: "vertical" | "horizontal", children?: array)
    Vertical (default) or horizontal flex stack of child components. `children` is an array of component refs, e.g. Stack("vertical", [a, b]).

- stat(value: binding, label?: string, unit?: string, tone?: "ok" | "warn" | "bad")
    A big single-value KPI tile with optional label, unit and tone.

- switch(value?: binding, label?: string)
    An on/off toggle; emits `toggle` with the next boolean state.
  actions: toggle

- table(rows: binding, columns?: array)
    A data table over `rows`; `columns` selects/orders fields (else inferred).

- tag(text: string, tone?: "ok" | "warn" | "bad")
    A small pill label with an optional tone. (`badge` is a deprecated alias.)

- text(value: string, muted?: boolean)
    A single paragraph of plain text; `muted` dims it.

- timeseries(rows: binding, xField?: string, yField?: string, color?: string, drawStyle?: "line" | "bars" | "points")
    A line/bars/points chart over `rows`; `yField` (+`xField`) select the numeric series.
```

<!-- END GENERATED CATALOG -->
