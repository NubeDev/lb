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

import { edit, history, post, remove } from "@/lib/channel/channel.api";
import { openChannelStream } from "@/lib/channel/channel.stream";
import { encodeAgent, encodeQuery, newRunId, type PageContext } from "@/lib/channel/payload.types";
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

/** Drop one id from a list (a live deletion reconciles the local view). */
function removeItem(items: Item[], id: string): Item[] {
  return items.filter((m) => m.id !== id);
}

export interface ChannelState {
  items: Item[];
  loading: boolean;
  error: string | null;
  send: (body: string) => Promise<void>;
  /** Edit the body of one of the caller's own messages (only the author may). */
  edit: (id: string, body: string) => Promise<void>;
  /** Delete one of the caller's own messages (only the author may). */
  remove: (id: string) => Promise<void>;
  /** Post a `kind:"query"` channel Item — the structured payload the host query worker answers. */
  postQuery: (source: string, sql: string) => Promise<void>;
  /** Post a `kind:"agent"` channel Item — the host agent worker drives the run and posts the answer
   *  back. `runtime` selects the agent (absent → in-house default; a profile id → an external agent).
   *  `context` (agent-dock scope) carries where the user is, fenced into the run's goal server-side. */
  postAgent: (goal: string, runtime?: string, context?: PageContext) => Promise<void>;
  /** Dispatch any other catalog tool via the host-mediated bridge (no channel Item). */
  callTool: (tool: string, args: Record<string, unknown>) => Promise<void>;
  /** Post a pre-encoded `kind:"rich_result"` render-envelope body as a channel Item (the palette posts a
   *  descriptor-declared interactive render this way, tool-agnostic — ResponseView renders it). */
  postRich: (body: string) => Promise<void>;
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
      onDelete: (id) => setItems((prev) => removeItem(prev, id)),
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

  // Post a `kind:"agent"` request. The UI mints the run id (so the card subscribes to the run stream
  // immediately) and builds the structured payload; the host agent worker answers it.
  //
  // The host now ENQUEUES a durable background run and returns at once (run-lifecycle #5) — the run is
  // driven off the POST connection by the host reactor, so it survives this tab closing AND a node
  // restart, and closing the tab no longer cancels it. We still fire-and-forget rather than await: the
  // request item, the live run feed, and the durable answer all arrive over SSE, so blocking the
  // composer buys nothing. `postBody` folds its own errors into `error`. In the Tauri shell / tests
  // (no SSE), the request appears when the post resolves and the answer when the run drains.
  const postAgent = useCallback(
    async (goal: string, runtime?: string, context?: PageContext) => {
      if (!goal.trim()) return;
      void postBody(encodeAgent(goal.trim(), newRunId(), runtime, context));
    },
    [postBody],
  );

  const send = useCallback(
    async (body: string) => {
      const trimmed = body.trim();
      if (!trimmed) return;
      // Plain chat. The agent is NO LONGER a `/agent` chat string parsed here — it is a first-class
      // palette command (`agent.invoke`) routed to `postAgent` by the CommandPalette, the same
      // mechanism `federation.query` uses (external-agent run-lifecycle #5). The host never parses
      // chat text for commands (channels-agent scope).
      await postBody(trimmed);
    },
    [postBody],
  );

  // Edit one of the caller's own messages, then reconcile against history. Only the author may
  // (the host re-checks ownership against the stored author); a denial surfaces as `error`.
  const editMessage = useCallback(
    async (id: string, body: string) => {
      const trimmed = body.trim();
      if (!trimmed) return;
      const ts = now();
      try {
        await edit(ws, channel, id, trimmed, ts);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [ws, channel, now, refresh],
  );

  // Delete one of the caller's own messages, then reconcile against history.
  const removeMessage = useCallback(
    async (id: string) => {
      try {
        await remove(ws, channel, id);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [ws, channel, refresh],
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

  // Post a `kind:"rich_result"` render-envelope Item (already encoded by the caller via
  // `encodeRichResult`) then reconcile against history — the same durable post path as chat/query, so a
  // structured response appears for every channel member and survives a reload.
  const postRich = useCallback(
    async (body: string) => {
      if (!body.trim()) return;
      await postBody(body);
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

  return {
    items,
    loading,
    error,
    send,
    edit: editMessage,
    remove: removeMessage,
    postQuery,
    postAgent,
    callTool,
    postRich,
  };
}
