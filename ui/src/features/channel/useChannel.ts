// The channel hook — data + state for one channel view (FILE-LAYOUT: one hook per file,
// data separated from markup). Loads history on mount, and posting appends optimistically
// then reconciles against the node's durable history (the source of truth, §3.3).
//
// "See it appear in real time": at S2 a post refreshes from history immediately. At S3 a live
// SSE feed (from the node's gateway) pushes OTHERS' messages into the SAME `setItems` sink — so
// the components don't change, only this hook gains a subscription. The merge is idempotent
// (upsert by id, kept ordered), exactly the node's contract, so a live item that also arrives
// via a later refresh never duplicates.

import { useCallback, useEffect, useState } from "react";

import { history, post } from "@/lib/channel/channel.api";
import { openChannelStream } from "@/lib/channel/channel.stream";
import { encodeQuery } from "@/lib/channel/payload.types";
import type { Item } from "@/lib/channel/channel.types";
import { invoke } from "@/lib/ipc/invoke";

/** Merge one item into a list: upsert by id, keep ordered by `ts` (the node's guarantees). */
function mergeItem(items: Item[], incoming: Item): Item[] {
  const next = items.slice();
  const at = next.findIndex((m) => m.id === incoming.id);
  if (at >= 0) next[at] = incoming;
  else next.push(incoming);
  next.sort((a, b) => a.ts - b.ts);
  return next;
}

export interface ChannelState {
  items: Item[];
  loading: boolean;
  error: string | null;
  send: (body: string) => Promise<void>;
  /** Post a `kind:"query"` channel Item — the structured payload the host query worker answers. */
  postQuery: (source: string, sql: string) => Promise<void>;
  /** Dispatch any other catalog tool via the host-mediated bridge (no channel Item). */
  callTool: (tool: string, args: Record<string, unknown>) => Promise<void>;
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

  // Live feed: push OTHERS' messages into the same `setItems` sink as they arrive (S3). Returns
  // null in the Tauri shell / tests (no gateway) — there the post→refresh round trip is the feed.
  useEffect(() => {
    const stream = openChannelStream(ws, channel, {
      onMessage: (item) => setItems((prev) => mergeItem(prev, item)),
    });
    return () => stream?.close();
  }, [ws, channel]);

  // Post one item body (chat text OR a structured payload JSON) then reconcile against history.
  const postBody = useCallback(
    async (body: string) => {
      const ts = now();
      const item: Item = { id: `${author}-${ts}`, channel, author, body, ts };
      try {
        await post(ws, channel, item);
        await refresh(); // reconcile against the durable history — the message appears now.
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [ws, channel, author, now, refresh],
  );

  const send = useCallback(
    async (body: string) => {
      const trimmed = body.trim();
      if (!trimmed) return;
      await postBody(trimmed);
    },
    [postBody],
  );

  // Post a `kind:"query"` Item — the host query worker sees it, runs federation.query, and posts a
  // `query_result`/`query_error` Item back (which streams in via the same history/SSE feed).
  const postQuery = useCallback(
    async (source: string, sql: string) => {
      if (!source || !sql.trim()) return;
      await postBody(encodeQuery(source, sql.trim()));
    },
    [postBody],
  );

  // Dispatch a non-query catalog tool through the host-mediated bridge (the same `mcp_call` seam the
  // federation client uses). The palette routes federation.query to `postQuery` instead.
  const callTool = useCallback(
    async (tool: string, args: Record<string, unknown>) => {
      try {
        await invoke("mcp_call", { tool, args });
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [refresh],
  );

  return { items, loading, error, send, postQuery, callTool };
}
