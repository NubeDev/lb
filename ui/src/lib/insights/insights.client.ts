// The shell's `InsightsClient` — the single wiring point that adapts the shell's transport (the
// `/mcp/call` bridge + the SSE event hub) onto the `@nube/insights` package's injected seam. The
// package's widgets (`InsightsReadWidget` / `InsightsAckWidget`) and hooks take this; that's how the
// dashboard reuses ONE insights implementation without the package importing `@/`.
//
// The package owns the vocabulary now: the shell's DTO/type module re-exports the package types (one
// shape, not two — see `insights.types.ts`). This adapter only maps method → verb.

import type { InsightsClient } from "@nube/insights";

import {
  ackInsight,
  getInsight,
  listInsights,
  listOccurrences,
  resolveInsight,
} from "./insights.api";
import { subscribeInsightEvents } from "./insights.events";

/** The shell's insights client — every method delegates to the existing `insight.*` api client; the
 *  optional live tail rides the SSE event hub (a no-op when no gateway is configured). */
export const insightsClient: InsightsClient = {
  list: (query) => listInsights(query),
  get: (id) => getInsight(id),
  ack: (id) => ackInsight(id),
  resolve: (id, note) => resolveInsight(id, note),
  occurrences: (insightId, cursor, limit) => listOccurrences(insightId, cursor, limit),
  subscribe: (onEvent) => subscribeInsightEvents(onEvent),
};
