// The `insights` per-viz options (insights-package-scope). The insights triage list is not source-
// bound; its config is purely presentation/filter, nested under `options.insights.*` (so a fresh cell's
// `defaultOptionsForView("insights")` block round-trips as one object). Two groups:
//   - Display: the READ-ONLY toggle (the headline affordance — off ⇒ end users ack/resolve/dismiss in
//     place) + the header refresh toggle;
//   - Filter: the status + severity facets + the row cap the widget's `filter` reads.
// Names/defaults match `views/insights/options.ts` (`readInsightsOptions`). One responsibility: the
// insights option catalog.

import type { OptionDef } from "../types";

const INSIGHTS = ["insights" as const];
const DISPLAY = "Display";
const FILTER = "Filter";

export const INSIGHTS_OPTIONS: OptionDef[] = [
  {
    id: "insights.readOnly",
    label: "Read only",
    group: DISPLAY,
    scope: "options",
    path: "insights.readOnly",
    views: INSIGHTS,
    control: { kind: "toggle" },
    default: true,
    keywords: ["acknowledge", "ack", "resolve", "dismiss", "interactive", "triage"],
  },
  {
    id: "insights.showRefresh",
    label: "Show refresh",
    group: DISPLAY,
    scope: "options",
    path: "insights.showRefresh",
    views: INSIGHTS,
    control: { kind: "toggle" },
    default: true,
  },
  {
    id: "insights.status",
    label: "Status",
    group: FILTER,
    scope: "options",
    path: "insights.status",
    views: INSIGHTS,
    control: {
      kind: "select",
      choices: [
        { value: "all", label: "All" },
        { value: "open", label: "Open" },
        { value: "acked", label: "Acknowledged" },
        { value: "resolved", label: "Resolved" },
      ],
    },
    default: "all",
  },
  {
    id: "insights.severity",
    label: "Severity",
    group: FILTER,
    scope: "options",
    path: "insights.severity",
    views: INSIGHTS,
    control: {
      kind: "select",
      choices: [
        { value: "all", label: "All" },
        { value: "info", label: "Info" },
        { value: "warning", label: "Warning" },
        { value: "critical", label: "Critical" },
      ],
    },
    default: "all",
  },
  {
    id: "insights.limit",
    label: "Max items",
    group: FILTER,
    scope: "options",
    path: "insights.limit",
    views: INSIGHTS,
    control: { kind: "number", min: 1, max: 200, step: 5 },
    default: 20,
    keywords: ["limit", "rows", "count"],
  },
];
