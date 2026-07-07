// The dock SESSION data path (agent-dock scope) — a thin wrapper over the shipped `useChannel` pointed
// at the current dock channel. History on open, live items over the channel SSE, and `ask()` posts a
// `kind:"agent"` item WITH the page context captured at send time. No new persistence/transport: the
// dock is a channel client (scope: "reuse the useChannel data path … pointed at the current dock
// channel"). One responsibility: bind useChannel to the dock + capture context per ask.
//
// Tools mode (agent-context-basket scope) reuses the SAME channel client: the dock's CommandPalette
// posts queries / rich renders / bridge calls through these pass-throughs, so a tool run in the dock
// is a durable dock-channel item — exactly what a context ref can then point at.

import { useCallback } from "react";

import { useChannel } from "@/features/channel/useChannel";
import type { Item } from "@/lib/channel/channel.types";
import { usePageContext } from "./PageContextProvider";

export interface DockSession {
  items: Item[];
  loading: boolean;
  error: string | null;
  /** Ask the active agent. Captures the CURRENT page context at send time (per-message) and posts a
   *  `kind:"agent"` item to the dock channel — the durable worker resolves the workspace's active
   *  runtime and posts `agent_result` back. `persona` (persona-session #5) is the dock chip's resolved
   *  per-tab focus; pass `undefined` to let the server's prefs fold decide. `contextItems`
   *  (agent-context-basket) are the basket's item refs — the worker resolves + fences them into the
   *  run's goal. `runtime` selects the agent for a palette-issued ask (absent → the workspace's
   *  active runtime, unchanged). */
  ask: (goal: string, persona?: string, contextItems?: string[], runtime?: string) => Promise<void>;
  /** Post a `kind:"query"` item to the dock channel (the palette's federation.query route). */
  postQuery: (source: string, sql: string) => Promise<void>;
  /** Post a pre-encoded `kind:"rich_result"` render-envelope (a descriptor-declared response). */
  postRich: (body: string) => Promise<void>;
  /** Dispatch any other catalog tool via the host-mediated bridge. */
  callTool: (tool: string, args: Record<string, unknown>) => Promise<void>;
  /** Send a plain chat note into the dock session (the palette's no-command path). */
  send: (body: string) => Promise<void>;
}

/** Drive the current dock session `(ws, cid)` as `author`. `now` is injectable for deterministic
 *  tests (threaded into the underlying channel post timestamps). */
export function useDockSession(
  ws: string,
  cid: string,
  author: string,
  now?: () => number,
): DockSession {
  const channel = useChannel(ws, cid, author, now);
  const page = usePageContext();

  const ask = useCallback(
    async (goal: string, persona?: string, contextItems?: string[], runtime?: string) => {
      const trimmed = goal.trim();
      if (!trimmed) return;
      // Capture the page context NOW (per-message): ask → navigate → ask carries the new page on the
      // second message. The dock passes NO runtime unless the palette picked one — the worker rides
      // the workspace's active agent. `persona` is the chip's resolved id (pin or context match) —
      // the chip and the payload must never disagree (one gateway test pins this). Undefined ⇒ the
      // server folds prefs to a default. `contextItems` are refs into THIS channel; the worker
      // resolves the bodies server-side (refs, not bodies, on the wire).
      await channel.postAgent(trimmed, runtime, page.capture(), persona, contextItems);
    },
    [channel, page],
  );

  return {
    items: channel.items,
    loading: channel.loading,
    error: channel.error,
    ask,
    postQuery: channel.postQuery,
    postRich: channel.postRich,
    callTool: channel.callTool,
    send: channel.send,
  };
}
