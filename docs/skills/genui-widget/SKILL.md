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

<!-- prose sections filled by the implementing session below; the catalog block is GENERATED -->

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
