// The in-memory inbox fake (TEST-ONLY) — mirrors the gateway `inbox_list` / `inbox_resolve` over the
// REAL durable inbox contract (not the workflow fake's simulated one). Workspace-scoped via the
// session store. A test seeds items with `__seedInboxItem`; `inbox_resolve` records the decision (the
// actor is the session principal, as the real host forces). Returns `null` for unowned commands.

import type { Item } from "@/lib/channel/channel.types";
import { getSession } from "@/lib/session/session.store";

interface Resolution {
  decision: string;
  actor: string;
}

const items = new Map<string, Map<string, Item[]>>(); // ws → (channel → items)
const resolutions = new Map<string, Map<string, Resolution>>(); // ws → (itemId → resolution)

function ws(): string {
  return getSession()?.workspace ?? "";
}

/** Test helper: seed a real durable inbox item in `workspace`. */
export function __seedInboxItem(workspace: string, item: Item): void {
  const byChannel = items.get(workspace) ?? new Map<string, Item[]>();
  const list = byChannel.get(item.channel) ?? [];
  list.push(item);
  list.sort((a, b) => a.ts - b.ts);
  byChannel.set(item.channel, list);
  items.set(workspace, byChannel);
}

/** Test helper: read a recorded resolution (mirrors the host's `resolution` read). */
export function __inboxResolution(workspace: string, itemId: string): Resolution | undefined {
  return resolutions.get(workspace)?.get(itemId);
}

export function inboxFakeInvoke<T>(cmd: string, args?: Record<string, unknown>): T | null {
  switch (cmd) {
    case "inbox_list": {
      const { channel } = args as { channel: string };
      return [...(items.get(ws())?.get(channel) ?? [])] as T;
    }
    case "inbox_resolve": {
      const { item, decision } = args as { item: string; decision: string };
      const byItem = resolutions.get(ws()) ?? new Map<string, Resolution>();
      byItem.set(item, { decision, actor: getSession()?.principal ?? "user:test" });
      resolutions.set(ws(), byItem);
      return undefined as T;
    }
    default:
      return null;
  }
}

/** Test helper: clear the fake inbox between tests. */
export function __resetInboxFake(): void {
  items.clear();
  resolutions.clear();
}
