// The inbox api client — one call per export, mirroring `lb_host::list_inbox` / `resolve_inbox` and
// the gateway `GET /inbox/{channel}` / `POST /inbox/{item}/resolve` (collaboration scope, slice 4).
// This is the REAL durable inbox; it replaces the workflow fake's simulated inbox on the real path.

import type { Decision, Item } from "./inbox.types";
import { invoke } from "@/lib/ipc/invoke";

/** List the durable items of inbox `channel`, oldest→newest. Mirrors `inbox_list`. */
export function listInbox(channel: string): Promise<Item[]> {
  return invoke<Item[]>("inbox_list", { channel });
}

/** Record a reviewer's `decision` on item `item`. Mirrors `inbox_resolve` (the S6 approval gate as
 *  a real UI action). */
export function resolveInbox(item: string, decision: Decision): Promise<void> {
  return invoke<void>("inbox_resolve", { item, decision });
}
