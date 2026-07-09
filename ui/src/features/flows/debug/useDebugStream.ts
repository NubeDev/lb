// The debug-panel state hook (debug-node-scope) — tails `openFlowDebugStream` for the open flow and
// holds the last N messages in a capped ring (v1 is motion-only: component state, never persisted).
// One responsibility: subscribe + buffer + clear; rendering lives in DebugPanel/DebugMessageRow.

import { useCallback, useEffect, useState } from "react";

import { openFlowDebugStream, type DebugMessage } from "@/lib/flows";

/** The ring cap — enough scrollback to be useful, bounded so a chatty flow can't grow the DOM
 *  without limit (the publish governor already rate-limits host-side). */
const MAX_MESSAGES = 200;

export interface DebugStreamState {
  /** Oldest → newest, capped at MAX_MESSAGES. */
  messages: DebugMessage[];
  /** False when no gateway/EventSource is available (Tauri/tests) — render "stream unavailable". */
  available: boolean;
  clear: () => void;
}

export function useDebugStream(flowId: string): DebugStreamState {
  const [messages, setMessages] = useState<DebugMessage[]>([]);
  const [available, setAvailable] = useState(true);

  useEffect(() => {
    setMessages([]);
    const stream = openFlowDebugStream(flowId, (msg) =>
      setMessages((ms) => [...ms.slice(-(MAX_MESSAGES - 1)), msg]),
    );
    setAvailable(stream !== null);
    return () => stream?.close();
  }, [flowId]);

  const clear = useCallback(() => setMessages([]), []);

  return { messages, available, clear };
}
