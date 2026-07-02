// The run-feed hook (channels-agent scope) — subscribes to a run's live `RunEvent` stream and folds it
// into a compact, render-ready shape for the AgentCard: the current reasoning/answer text and the tool
// calls seen so far. One hook per file (FILE-LAYOUT); the SSE plumbing lives in `run.stream.ts`.
//
// `active` gates the subscription: the card only watches WHILE a run is pending (no durable
// `agent_result` yet). Once settled, the card stops watching and shows the durable answer.

import { useEffect, useState } from "react";

import { openRunStream, type RunEvent } from "@/lib/channel/run.stream";

/** One tool the agent invoked during the run, in the order first seen. */
export interface RunToolCall {
  id: string;
  name: string;
  /** null while running; set to ok/err text when the result arrives. */
  ok?: string | null;
  err?: string | null;
}

export interface RunFeed {
  /** True once the stream is open (a real gateway); false in the Tauri shell / tests. */
  live: boolean;
  /** Assistant-visible text accumulated from `text-delta`s so far. */
  text: string;
  /** The latest reasoning line (from `reasoning-delta`), shown muted while the agent thinks. */
  reasoning: string;
  /** Tool calls in first-seen order, each updated in place when its result lands. */
  tools: RunToolCall[];
  /** True once a `run-finish` arrived over the stream. */
  finished: boolean;
}

const EMPTY: RunFeed = { live: false, text: "", reasoning: "", tools: [], finished: false };

/** Watch run `job` while `active`, folding its `RunEvent`s into a {@link RunFeed}. Closes the stream
 *  on unmount or when `active` goes false (the durable answer superseded it). */
export function useRunFeed(job: string, active: boolean): RunFeed {
  const [feed, setFeed] = useState<RunFeed>(EMPTY);

  useEffect(() => {
    if (!active) return;
    const stream = openRunStream(job, (event) => setFeed((prev) => fold(prev, event)));
    if (stream) setFeed((prev) => ({ ...prev, live: true }));
    return () => stream?.close();
  }, [job, active]);

  return feed;
}

/** Fold one RunEvent into the feed — pure, so it is trivially testable. */
export function fold(feed: RunFeed, event: RunEvent): RunFeed {
  switch (event.type) {
    case "text-delta":
      return { ...feed, text: feed.text + event.text };
    case "reasoning-delta":
      return { ...feed, reasoning: event.text };
    case "tool-call-start":
      // First-seen wins; a duplicate id (start echoed) doesn't add a second row.
      if (feed.tools.some((t) => t.id === event.id)) return feed;
      return { ...feed, tools: [...feed.tools, { id: event.id, name: event.name }] };
    case "tool-call-result":
      return {
        ...feed,
        tools: feed.tools.map((t) =>
          t.id === event.id ? { ...t, ok: event.ok ?? null, err: event.err ?? null } : t,
        ),
      };
    case "run-finish":
      // Prefer the finish answer when the stream carried no text deltas (the per-step transport).
      return { ...feed, finished: true, text: feed.text || event.answer };
    default:
      return feed; // run-start / step-start / args-delta carry nothing the card renders yet
  }
}
