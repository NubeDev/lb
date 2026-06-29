// The channel API client — one call per export, mirroring the Rust channel verbs
// (`host::post`, `host::history`) and the Tauri command names one-to-one. The UI never calls
// `invoke` directly; it goes through these named verbs (FILE-LAYOUT frontend rules).

import type { ChannelRecord, Item } from "./channel.types";
import type { ToolsCatalog } from "./palette.types";
import { invoke } from "@/lib/ipc/invoke";

/** Read the calling principal's authorized tool catalog for their workspace (the command-palette's
 *  one read). Reached over the same MCP bridge as any verb (rule 7): `tools.catalog` returns
 *  `{ ws, tools }` — registered tools ∩ caps held. Mirrors `lb_host::tools_catalog`. */
export function toolsCatalog(): Promise<ToolsCatalog> {
  return invoke<ToolsCatalog>("mcp_call", { tool: "tools.catalog", args: {} });
}

/** Post `item` to `channel` in workspace `ws`. Returns the stored item (channel filled in).
 *  Mirrors `lb_host::post`. */
export function post(ws: string, channel: string, item: Item): Promise<Item> {
  return invoke<Item>("channel_post", { ws, channel, item });
}

/** Read `channel`'s durable history in workspace `ws`, oldest→newest. Mirrors
 *  `lb_host::history`. */
export function history(ws: string, channel: string): Promise<Item[]> {
  return invoke<Item[]>("channel_history", { ws, channel });
}

/** Edit the body of one of the caller's own messages in `channel`. Only the message's author may
 *  edit it (the host re-checks ownership against the stored author). `ts` is the new logical
 *  ordering timestamp (caller-injected, like `Item.ts`). Mirrors `lb_host::edit`. */
export function edit(
  ws: string,
  channel: string,
  id: string,
  body: string,
  ts: number,
): Promise<Item> {
  return invoke<Item>("channel_edit", { ws, channel, id, body, ts });
}

/** Delete one of the caller's own messages in `channel`. Only the message's author may delete it.
 *  Mirrors `lb_host::delete`. */
export function remove(ws: string, channel: string, id: string): Promise<void> {
  return invoke<void>("channel_delete", { ws, channel, id });
}

/** List the registered channels in workspace `ws` (for the switcher). Mirrors `channel_list`. */
export function listChannels(ws: string): Promise<ChannelRecord[]> {
  return invoke<ChannelRecord[]>("channel_list", { ws });
}

/** Explicitly register `channel` in workspace `ws` so it is listable. Mirrors `channel_create`. */
export function createChannel(ws: string, channel: string): Promise<ChannelRecord> {
  return invoke<ChannelRecord>("channel_create", { ws, channel });
}
