// The in-memory node stand-in used when NOT in the Tauri shell (plain browser during S2, and
// tests). It mirrors the real node's channel contract: `channel_post` persists + returns the
// stored item; `channel_history` returns the channel's items oldest→newest. This is a
// temporary seam — at S3 the browser talks to a real node over SSE/HTTP and this is dropped.
//
// It is deliberately faithful (ordered, idempotent on id) so the UI behaves identically here
// and against the real node — the verb names and shapes match the Rust commands one-to-one.

import type { Item } from "@/lib/channel/channel.types";
import { assetsFakeInvoke } from "./assets.fake";

const store = new Map<string, Item[]>(); // key: `${ws}/${channel}`

function key(ws: string, channel: string): string {
  return `${ws}/${channel}`;
}

export function fakeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  // Asset commands (`assets_*`) are handled by the asset fake; it returns null for anything
  // it doesn't own, so the channel cases below still run.
  const asset = assetsFakeInvoke<T>(cmd, args);
  if (asset !== null) return asset;
  switch (cmd) {
    case "channel_post": {
      const { ws, channel, item } = args as {
        ws: string;
        channel: string;
        item: Item;
      };
      const list = store.get(key(ws, channel)) ?? [];
      const stored: Item = { ...item, channel };
      const existing = list.findIndex((m) => m.id === stored.id);
      if (existing >= 0) list[existing] = stored;
      else list.push(stored);
      list.sort((a, b) => a.ts - b.ts);
      store.set(key(ws, channel), list);
      return Promise.resolve(stored as T);
    }
    case "channel_history": {
      const { ws, channel } = args as { ws: string; channel: string };
      const list = store.get(key(ws, channel)) ?? [];
      return Promise.resolve([...list] as T);
    }
    default:
      return Promise.reject(new Error(`unknown command: ${cmd}`));
  }
}

/** Test helper: clear the fake store between tests (the fake is module-global). */
export function __resetFake(): void {
  store.clear();
}
