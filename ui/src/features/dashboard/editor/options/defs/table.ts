// The `table` per-viz + per-column field options (editor-parity scope, step 5 — table was the deepest
// gap, a 25-line editor). Two scopes:
//   - per-VIZ `options.*` (showHeader / cellHeight / enablePagination / footer reducers) — how the
//     table draws as a whole;
//   - per-FIELD `custom.*` (column width, alignment, cell display type, per-column filter) — Grafana's
//     `TableFieldOptions`, so they live in fieldConfig and back an override, exactly like Grafana.
// Names + defaults verbatim from Grafana's table panelcfg. One responsibility: the table option catalog.

import type { OptionDef } from "../types";

const TABLE = ["table" as const];
const OPTS = "Table options";
const CELL = "Cell";

export const TABLE_OPTIONS: OptionDef[] = [
  // --- per-viz options ---
  { id: "showHeader", label: "Show header", group: OPTS, scope: "options", views: TABLE, control: { kind: "toggle" }, default: true },
  {
    id: "cellHeight",
    label: "Cell height",
    group: OPTS,
    scope: "options",
    views: TABLE,
    control: { kind: "select", choices: [{ value: "sm", label: "Small" }, { value: "md", label: "Medium" }, { value: "lg", label: "Large" }] },
    default: "sm",
  },
  { id: "enablePagination", label: "Enable pagination", group: OPTS, scope: "options", views: TABLE, control: { kind: "toggle" }, default: false },
  {
    id: "footer.show",
    label: "Show table footer",
    group: "Footer",
    scope: "options",
    path: "footer.show",
    views: TABLE,
    control: { kind: "toggle" },
    default: false,
    keywords: ["total", "sum", "aggregate"],
  },
  {
    id: "footer.reducer",
    label: "Footer calculation",
    group: "Footer",
    scope: "options",
    path: "footer.reducer",
    views: TABLE,
    control: { kind: "select", choices: [{ value: "sum" }, { value: "mean" }, { value: "min" }, { value: "max" }, { value: "count" }] },
    default: "sum",
  },

  // --- per-column field options (custom.*) — Grafana's TableFieldOptions ---
  {
    id: "custom.width",
    label: "Column width",
    group: CELL,
    scope: "fieldConfig",
    views: TABLE,
    control: { kind: "number", min: 20, max: 800, step: 10, placeholder: "auto" },
    default: undefined,
  },
  {
    id: "custom.align",
    label: "Column alignment",
    group: CELL,
    scope: "fieldConfig",
    views: TABLE,
    control: { kind: "select", choices: [{ value: "auto" }, { value: "left" }, { value: "center" }, { value: "right" }] },
    default: "auto",
  },
  {
    id: "custom.cellOptions.type",
    label: "Cell type",
    group: CELL,
    scope: "fieldConfig",
    views: TABLE,
    control: {
      kind: "select",
      choices: [
        { value: "auto", label: "Auto" },
        { value: "color-text", label: "Colored text" },
        { value: "color-background", label: "Colored background" },
        { value: "gauge", label: "Gauge" },
        { value: "sparkline", label: "Sparkline" },
      ],
    },
    default: "auto",
    keywords: ["background", "gauge", "sparkline", "color"],
  },
  {
    id: "custom.filterable",
    label: "Column filter",
    group: CELL,
    scope: "fieldConfig",
    views: TABLE,
    control: { kind: "toggle" },
    default: false,
    keywords: ["filter", "search"],
  },
];
