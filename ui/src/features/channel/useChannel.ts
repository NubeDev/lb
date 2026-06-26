// The channel hook — data + state for one channel view (FILE-LAYOUT: one hook per file,
// data separated from markup). Loads history on mount, and posting appends optimistically
// then reconciles against the node's durable history (the source of truth, §3.3).
//
// "See it appear in real time": at S2 a post refreshes from history immediately, so the
// message shows the moment it lands. At S3 a live SSE/bus feed will push others' messages
// here too — the same `setItems` sink, so the components don't change.

import { useCallback, useEffect, useState } from "react";

import { history, post } from "@/lib/channel/channel.api";
import type { Item } from "@/lib/channel/channel.types";

export interface ChannelState {
  items: Item[];
  loading: boolean;
  error: string | null;
  send: (body: string) => Promise<void>;
}

/** Drive a channel view for `(ws, channel)` as `author`. `now` injects the logical
 *  timestamp (kept injectable so tests stay deterministic — no wall-clock in logic). */
export function useChannel(
  ws: string,
  channel: string,
  author: string,
  now: () => number = () => Date.now(),
): ChannelState {
  const [items, setItems] = useState<Item[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setItems(await history(ws, channel));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [ws, channel]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const send = useCallback(
    async (body: string) => {
      const trimmed = body.trim();
      if (!trimmed) return;
      const ts = now();
      const item: Item = {
        id: `${author}-${ts}`,
        channel,
        author,
        body: trimmed,
        ts,
      };
      try {
        await post(ws, channel, item);
        await refresh(); // reconcile against the durable history — the message appears now.
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [ws, channel, author, now, refresh],
  );

  return { items, loading, error, send };
}
