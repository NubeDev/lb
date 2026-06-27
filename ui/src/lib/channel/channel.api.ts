// The channel API client — one call per export, mirroring the Rust channel verbs
// (`host::post`, `host::history`) and the Tauri command names one-to-one. The UI never calls
// `invoke` directly; it goes through these named verbs (FILE-LAYOUT frontend rules).

import type { ChannelRecord, Item } from "./channel.types";
import { invoke } from "@/lib/ipc/invoke";

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

/** List the registered channels in workspace `ws` (for the switcher). Mirrors `channel_list`. */
export function listChannels(ws: string): Promise<ChannelRecord[]> {
  return invoke<ChannelRecord[]>("channel_list", { ws });
}

/** Explicitly register `channel` in workspace `ws` so it is listable. Mirrors `channel_create`. */
export function createChannel(ws: string, channel: string): Promise<ChannelRecord> {
  return invoke<ChannelRecord>("channel_create", { ws, channel });
}
