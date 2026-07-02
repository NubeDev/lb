// The v1 nube catalog — the ONLY set of components an agent may emit (genui-scope "The v1 catalog is
// small and honest"). One `defineCatalog([...])` drives BOTH prompt surfaces and the React render fns.
//
// Every render fn is DEFENSIVE (coerce/guard every prop, never throw) and satisfies the promotion
// checklist:
//   1+2. NO `dangerouslySetInnerHTML` anywhere; markdown is tokenized to React elements with NO raw
//        HTML pass-through, and only http/https/mailto links become <a href>.
//   3.   No prop is evaluated as code (no eval / new Function / expression props).
//   4.   Controls emit ONLY through `rp.emit(name, context)` — never a direct fetch/DOM escape.
//   5.   Enum props map to a FIXED whitelist of `gu-*` classes; the only per-prop style is a numeric
//        CSS var we clamp ourselves (grid `--gu-cols`). No user string ever reaches className/style.

import type { ReactNode } from "react";
import type { Catalog, RenderProps } from "./defineCatalog";
import { defineCatalog } from "./defineCatalog";
import { renderMarkdown } from "./markdown";
import { Barchart, Gauge, Piechart, Timeseries } from "./charts";

// ── coercion helpers (local, tiny; NOT a utils grab-bag) ──────────────────────────────────────────
function str(v: unknown, fallback = ""): string {
  return typeof v === "string" ? v : v == null ? fallback : String(v);
}
function num(v: unknown, fallback = NaN): number {
  const n = typeof v === "number" ? v : Number(v);
  return Number.isFinite(n) ? n : fallback;
}
/** Map a tone prop to a FIXED class via whitelist — never spread the raw value. */
function toneClass(v: unknown): string {
  return v === "ok" || v === "warn" || v === "bad" ? ` gu-tone-${v}` : "";
}

// ── the catalog ───────────────────────────────────────────────────────────────────────────────────
export const nubeCatalog: Catalog = defineCatalog([
  // ---- normalize target ----
  {
    // The component `normalize` rewrites an unknown name to (labelled placeholder + warning). Kept in
    // the catalog so a normalized spec renders the intended inert placeholder rather than the surface's
    // last-resort `gu-unknown` fallback. Not something the agent should emit directly.
    name: "placeholder",
    description: "Inert labelled placeholder (normalize target for an unknown component).",
    props: { label: { type: "string", description: "What was replaced." } },
    render: (rp: RenderProps) => <div className="gu-placeholder" role="status">{str(rp.props.label, "placeholder")}</div>,
  },
  // ---- layout ----
  {
    name: "stack",
    description: "Vertical (default) or horizontal flex stack of child components. `children` is an array of component refs, e.g. Stack(\"vertical\", [a, b]).",
    props: {
      direction: { type: "enum", values: ["vertical", "horizontal"], description: "Stack axis.", default: "vertical" },
      children: { type: "array", description: "Array of child component refs." },
    },
    render: (rp: RenderProps) => {
      const horizontal = rp.props.direction === "horizontal";
      return <div className={horizontal ? "gu-stack gu-horizontal" : "gu-stack"}>{rp.children}</div>;
    },
  },
  {
    name: "grid",
    description: "Responsive grid; `columns` (1..6) per row. `children` is an array of component refs, e.g. Grid(2, [a, b, c]).",
    props: {
      columns: { type: "number", description: "Columns per row, clamped 1..6.", default: 2 },
      children: { type: "array", description: "Array of child component refs." },
    },
    render: (rp: RenderProps) => {
      const cols = Math.max(1, Math.min(6, Math.round(num(rp.props.columns, 2))));
      // Only a numeric CSS var we control is injected — never a user string.
      return (
        <div className="gu-grid" style={{ ["--gu-cols" as string]: String(cols) }}>
          {rp.children}
        </div>
      );
    },
  },
  {
    name: "card",
    description: "A bordered card with an optional title, wrapping child components. `children` is an array of component refs, e.g. Card(\"Room\", [a, b]).",
    props: {
      title: { type: "string", description: "Optional card heading." },
      children: { type: "array", description: "Array of child component refs." },
    },
    render: (rp: RenderProps) => {
      const title = str(rp.props.title);
      return (
        <div className="gu-card">
          {title && <div className="gu-card-title">{title}</div>}
          {rp.children}
        </div>
      );
    },
  },

  // ---- text ----
  {
    name: "text",
    description: "A single paragraph of plain text; `muted` dims it.",
    props: {
      value: { type: "string", required: true, description: "The text to display." },
      muted: { type: "boolean", description: "Render dimmed/secondary." },
    },
    render: (rp: RenderProps) => {
      const muted = rp.props.muted === true;
      return (
        <p className="gu-text" style={muted ? { color: "var(--gu-muted)" } : undefined}>
          {str(rp.props.value)}
        </p>
      );
    },
  },
  {
    name: "markdown",
    description: "Safe minimal markdown (headings, bold, italic, code, links, lists) — no raw HTML.",
    props: {
      value: { type: "string", required: true, description: "Markdown source text." },
    },
    render: (rp: RenderProps) => <div className="gu-markdown">{renderMarkdown(str(rp.props.value))}</div>,
  },

  // ---- stat / gauge ----
  {
    name: "stat",
    description: "A big single-value KPI tile with optional label, unit and tone.",
    props: {
      value: { type: "binding", required: true, description: "The KPI value (number or string)." },
      label: { type: "string", description: "Caption above the value." },
      unit: { type: "string", description: "Unit suffix appended to the value." },
      tone: { type: "enum", values: ["ok", "warn", "bad"], description: "Colour tone of the value." },
    },
    render: (rp: RenderProps) => {
      const label = str(rp.props.label);
      const unit = str(rp.props.unit);
      const raw = rp.props.value;
      const display = typeof raw === "number" ? String(raw) : str(raw, "—");
      return (
        <div className={`gu-stat${toneClass(rp.props.tone)}`}>
          {label && <div className="gu-stat-label">{label}</div>}
          <div className="gu-stat-value">
            {display}
            {unit ? ` ${unit}` : ""}
          </div>
        </div>
      );
    },
  },
  {
    name: "gauge",
    description: "A radial arc gauge showing `value` between `min`/`max` with optional thresholds.",
    props: {
      value: { type: "binding", required: true, description: "Current value." },
      min: { type: "number", description: "Range minimum.", default: 0 },
      max: { type: "number", description: "Range maximum.", default: 100 },
      thresholds: { type: "array", description: "Ascending numeric warn/bad thresholds." },
    },
    render: (rp: RenderProps) => Gauge(rp.props),
  },

  // ---- table ----
  {
    name: "table",
    description: "A data table over `rows`; `columns` selects/orders fields (else inferred).",
    props: {
      rows: { type: "binding", required: true, description: "Array of row objects." },
      columns: { type: "array", description: "Column keys to show, in order." },
    },
    render: (rp: RenderProps) => renderTable(rp.props),
  },

  // ---- charts ----
  {
    name: "timeseries",
    description: "A line/bars/points chart over `rows`; `yField` (+`xField`) select the numeric series.",
    props: {
      rows: { type: "binding", required: true, description: "Array of data rows." },
      xField: { type: "string", description: "Row field for the x axis (else index)." },
      yField: { type: "string", description: "Row field for the y value." },
      color: { type: "string", description: "Reserved; series colour follows the theme." },
      drawStyle: { type: "enum", values: ["line", "bars", "points"], description: "How to draw the series." },
    },
    render: (rp: RenderProps) => Timeseries(rp.props),
  },
  {
    name: "barchart",
    description: "A bar chart over `data` ({name,value}[]); `horizontal` flips the orientation.",
    props: {
      data: { type: "binding", required: true, description: "Array of {name, value} items." },
      horizontal: { type: "boolean", description: "Draw horizontal bars." },
    },
    render: (rp: RenderProps) => Barchart(rp.props),
  },
  {
    name: "piechart",
    description: "A pie/donut chart over `data` ({name,value}[]).",
    props: {
      data: { type: "binding", required: true, description: "Array of {name, value} items." },
      pieType: { type: "enum", values: ["pie", "donut"], description: "Solid pie or donut." },
    },
    render: (rp: RenderProps) => Piechart(rp.props),
  },

  // ---- tag / badge ----
  {
    name: "tag",
    description: "A small pill label with an optional tone. (`badge` is a deprecated alias.)",
    deprecatedAliases: ["badge"],
    props: {
      text: { type: "string", required: true, description: "The label text." },
      tone: { type: "enum", values: ["ok", "warn", "bad"], description: "Colour tone." },
    },
    render: (rp: RenderProps) => <span className={`gu-tag${toneClass(rp.props.tone)}`}>{str(rp.props.text)}</span>,
  },

  // ---- controls (emit ONLY via rp.emit) ----
  {
    name: "button",
    description: "A button; emits `press` with its value when clicked.",
    actions: ["press"],
    props: {
      label: { type: "string", required: true, description: "Button caption." },
      value: { type: "binding", description: "Value carried in the emitted action." },
    },
    render: (rp: RenderProps) => (
      <button type="button" className="gu-btn" onClick={() => rp.emit("press", { value: rp.props.value ?? null })}>
        {str(rp.props.label, "Button")}
      </button>
    ),
  },
  {
    name: "slider",
    description: "A range slider; emits `change` with the committed value on release.",
    actions: ["change"],
    props: {
      value: { type: "binding", description: "Initial/display value.", default: 0 },
      min: { type: "number", description: "Minimum.", default: 0 },
      max: { type: "number", description: "Maximum.", default: 100 },
      step: { type: "number", description: "Step size.", default: 1 },
      label: { type: "string", description: "Optional caption." },
    },
    render: (rp: RenderProps) => renderSlider(rp),
  },
  {
    name: "switch",
    description: "An on/off toggle; emits `toggle` with the next boolean state.",
    actions: ["toggle"],
    props: {
      value: { type: "binding", description: "Current on/off state.", default: false },
      label: { type: "string", description: "Optional caption." },
    },
    render: (rp: RenderProps) => {
      const on = rp.props.value === true;
      const label = str(rp.props.label);
      return (
        <label className="gu-switch">
          <input type="checkbox" checked={on} onChange={() => rp.emit("toggle", { value: !on })} />
          {label && <span>{label}</span>}
        </label>
      );
    },
  },
]);

// ── table renderer (kept out of the entry for readability) ────────────────────────────────────────
function renderTable(props: Record<string, unknown>): ReactNode {
  const rows = Array.isArray(props.rows) ? (props.rows as unknown[]) : [];
  if (rows.length === 0) return <div className="gu-placeholder">no data</div>;
  const explicit = Array.isArray(props.columns)
    ? (props.columns as unknown[]).map((c) => str(c)).filter(Boolean)
    : [];
  const cols =
    explicit.length > 0
      ? explicit
      : Array.from(
          rows.reduce<Set<string>>((set, r) => {
            if (r && typeof r === "object") for (const k of Object.keys(r as object)) set.add(k);
            return set;
          }, new Set<string>()),
        );
  if (cols.length === 0) return <div className="gu-placeholder">no data</div>;
  return (
    <table className="gu-table">
      <thead>
        <tr>
          {cols.map((c) => (
            <th key={c}>{c}</th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((r, i) => (
          <tr key={i}>
            {cols.map((c) => {
              const cell = r && typeof r === "object" ? (r as Record<string, unknown>)[c] : undefined;
              return <td key={c}>{cell == null ? "" : typeof cell === "object" ? JSON.stringify(cell) : String(cell)}</td>;
            })}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

// ── slider renderer (uncontrolled display seeded from props.value; emits only on commit) ───────────
function renderSlider(rp: RenderProps): ReactNode {
  const min = num(rp.props.min, 0);
  const max = num(rp.props.max, 100);
  const step = num(rp.props.step, 1);
  const seed = num(rp.props.value, min);
  const label = str(rp.props.label);
  const commit = (e: { currentTarget: { value: string } }) => {
    const v = num(e.currentTarget.value, seed);
    rp.emit("change", { value: v });
  };
  return (
    <label className="gu-switch">
      {label && <span>{label}</span>}
      <input
        type="range"
        className="gu-range"
        min={Number.isFinite(min) ? min : 0}
        max={Number.isFinite(max) ? max : 100}
        step={Number.isFinite(step) && step > 0 ? step : 1}
        defaultValue={Number.isFinite(seed) ? seed : min}
        onMouseUp={commit}
        onKeyUp={commit}
      />
    </label>
  );
}
