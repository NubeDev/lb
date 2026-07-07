// The context-basket PURE ops (agent-context-basket scope) — the logic behind "gather items, feed
// them to the next ask". Kept pure (no React) so the toggle/label rules are unit-testable; the thin
// state hook (`useContextBasket`) and the chips (`DockContextBasket`) consume these.

import type { Item } from "@/lib/channel/channel.types";
import { parsePayload } from "@/lib/channel/payload.types";

/** The most refs one request may carry — mirrors the host's `MAX_CONTEXT_ITEMS` (over-cap is a
 *  server-side reject, so the UI simply refuses to add past it). */
export const MAX_CONTEXT_ITEMS = 8;

/** Toggle `id` in the ordered ref list. Adding past {@link MAX_CONTEXT_ITEMS} is a no-op (the host
 *  would reject the request; the basket never builds an unsendable ask). */
export function toggleRef(ids: string[], id: string): string[] {
  if (ids.includes(id)) return ids.filter((x) => x !== id);
  if (ids.length >= MAX_CONTEXT_ITEMS) return ids;
  return [...ids, id];
}

/** A short human label for a basket chip: the referenced item's payload kind ("query result",
 *  "response", …) or a chat snippet — the id alone is meaningless to a user. Unknown ref → the id. */
export function refLabel(items: Item[], id: string): string {
  const item = items.find((m) => m.id === id);
  if (!item) return id;
  const payload = parsePayload(item.body);
  switch (payload?.kind) {
    case "query_result":
      return "query result";
    case "query_error":
      return "query error";
    case "rich_result":
      return "response";
    case "agent_result":
      return "agent answer";
    case "query":
      return "query";
    case "agent":
    case "agent_error":
      return payload.kind.replace("_", " ");
    default: {
      const text = item.body.trim();
      return text.length > 24 ? `${text.slice(0, 24)}…` : text || id;
    }
  }
}
