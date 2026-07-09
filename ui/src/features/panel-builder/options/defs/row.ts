// The `row` layout-view options (panel-rows scope). A row header carries no fieldConfig and no data —
// its config is purely presentation, nested under `options.*`. Three toggles under one Row group:
//   - Show count   → the "· N panels" member count beside the title;
//   - Show line    → the bottom divider line under the bar;
//   - Collapsed    → the DEFAULT open/closed state (the stored `options.collapsed`, applied on load).
// Names/defaults match `lib/dashboard/rows.ts` (`rowOptions`) and the `row` entry in the host
// `widget_catalog.json`. One responsibility: the row option catalog.

import type { OptionDef } from "../types";

const ROW = ["row" as const];
const GROUP = "Row";

export const ROW_OPTIONS: OptionDef[] = [
  {
    id: "showCount",
    label: "Show panel count",
    group: GROUP,
    scope: "options",
    path: "showCount",
    views: ROW,
    control: { kind: "toggle" },
    default: true,
    keywords: ["count", "panels", "members"],
  },
  {
    id: "showLine",
    label: "Show divider line",
    group: GROUP,
    scope: "options",
    path: "showLine",
    views: ROW,
    control: { kind: "toggle" },
    default: true,
    keywords: ["line", "border", "divider", "rule"],
  },
  {
    id: "collapsed",
    label: "Collapsed by default",
    group: GROUP,
    scope: "options",
    path: "collapsed",
    views: ROW,
    control: { kind: "toggle" },
    default: false,
    keywords: ["open", "closed", "expand", "fold", "default"],
  },
];
