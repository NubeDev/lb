// The insights api client — one call per export, mirroring `lb_host::insight_*` and the gateway
// `GET /insights/*` routes (insights umbrella scope + occurrences sub-scope). The raise verb +
// subscriptions + policy reach through the universal MCP `invoke` bridge; the page's primary
// REST surface (list/get/ack/resolve/occurrences) has dedicated routes here.

import { invoke } from "@/lib/ipc/invoke";
import type {
  Insight,
  ListPage,
  ListQuery,
  OccCursor,
  OccurrencePage,
} from "./insights.types";

/** List insights newest-first, keyset-paged. Mirrors `GET /insights` (`insight_list`). */
export function listInsights(query: ListQuery): Promise<ListPage> {
  return invoke<ListPage>("list_insights", { ...query });
}

/** Read one insight by id. Mirrors `GET /insights/{id}` (`insight_get`). */
export function getInsight(id: string): Promise<Insight | null> {
  return invoke<Insight | null>("get_insight", { id });
}

/** Ack an insight (open → acked). Mirrors `POST /insights/{id}/ack` (`insight_ack`). */
export function ackInsight(id: string): Promise<void> {
  return invoke<void>("ack_insight", { id });
}

/** Resolve an insight (* → resolved). Mirrors `POST /insights/{id}/resolve` (`insight_resolve`). */
export function resolveInsight(id: string, note?: string): Promise<void> {
  return invoke<void>("resolve_insight", { id, note });
}

/** Read the per-insight occurrence ring, newest-first. Mirrors `GET /insights/{id}/occurrences`. */
export function listOccurrences(
  insightId: string,
  cursor?: OccCursor,
  limit?: number,
): Promise<OccurrencePage> {
  return invoke<OccurrencePage>("list_occurrences", {
    id: insightId,
    cursor,
    limit,
  });
}
