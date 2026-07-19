// `@nube/insights` — public surface.
//
// The Lazybones "triage findings raised by rules, flows, and agents" machinery, extracted from the
// dashboard so any surface reuses ONE implementation — the Insights page, a dashboard widget (read-only
// or acknowledge), a standalone extension UI. Transport-agnostic: the host injects an `InsightsClient`
// (how to reach the node's `insight.*` verbs) — the shell delegates to its `/mcp/call` bridge; an
// extension delegates to its host bridge; a widget shares the shell's.
//
// Consumed two ways:
//   - `workspace:*` from the lb `ui/` app (the page + dashboard widgets),
//   - imported by a standalone extension UI — `import { ... }` + `import '@nube/insights/style.css'`.
//
// Three layers, adopt what you need: the MODEL (pure), the HOOKS, the OPTIONAL UI. The look is optional
// — a host may bring its own skin and drive only the hooks + model.

import "./insights.css";

// Model (pure) — the look-free vocabulary: severity/status ordering + tone keys + formatters.
export {
  SEVERITY_ORDER,
  severityRank,
  severityTone,
  statusTone,
  timeAgo,
  originLine,
} from "./model";
export type { Tone } from "./model";

// Hooks — data + state over the injected client. The list drives a page/widget; the detail drives an
// expanded/investigate view.
export { useInsights } from "./useInsights";
export type { InsightsState } from "./useInsights";
export { useInsight } from "./useInsight";
export type { InsightDetailState } from "./useInsight";

// UI (optional look) — the dashboard widget (read-only + acknowledge presets), plus the row/badge/
// action primitives a host composes into its own layout.
export {
  InsightsWidget,
  InsightsReadWidget,
  InsightsAckWidget,
} from "./InsightsWidget";
export type { InsightsWidgetProps } from "./InsightsWidget";
export { InsightRow } from "./InsightRow";
export type { InsightRowProps } from "./InsightRow";
export { InsightActions } from "./InsightActions";
export type { InsightActionsProps } from "./InsightActions";
export { SeverityBadge, StatusBadge } from "./InsightBadge";

// Reference client — a real in-memory `InsightsClient` (+ a deny variant) for host demos and tests.
// Not a fake of node behaviour: the client IS the boundary the package is defined against.
export { memoryClient, denyClient } from "./memoryClient";

// Types — the vocabulary + the injected transport seam.
export type {
  Severity,
  Status,
  OriginKind,
  Origin,
  Evidence,
  EvidenceSeries,
  Insight,
  Occurrence,
  InsightEvent,
  PageCursor,
  ListFilter,
  ListQuery,
  ListPage,
  OccCursor,
  OccurrencePage,
  InsightsClient,
} from "./types";
