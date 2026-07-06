// The query-draft follow hook (query-draft-streaming scope) — subscribes the workbench to the
// draft subject for its source over the SHIPPED bus SSE (`openBusStream` → `GET /bus/stream`),
// parses each frame defensively (`parseDraftFrame`), and hands valid ones to the caller. One
// responsibility (FILE-LAYOUT): the subscription lifecycle. The caller (QueryWorkbench) owns what
// a frame *does* (replace its editor state).
//
// Honest degrade: no gateway configured (Tauri/tests), no EventSource, or a denied
// `mcp:bus.watch:call` (the stream route 403s and EventSource just stays silent/errors) ⇒ the
// hook does nothing — the workbench works exactly as before, it merely doesn't follow.

import { useEffect, useRef, useState } from "react";

import { openBusStream } from "@/lib/dashboard/bus.stream";
import type { SqlSourceState } from "@/lib/panel-kit/sql/query";
import { draftSubject, parseDraftFrame } from "./queryDraft";

/**
 * Follow live query-draft frames for `source`. Returns the epoch-ms of the last applied frame
 * (null until one arrives) — the workbench renders its "live draft" indicator from it.
 *
 * @param source The workbench source (`"surreal-local"` or a federation datasource name).
 * @param onFrame Called with each VALID frame (already parsed; malformed frames are dropped).
 */
export function useQueryDraftFollow(
  source: string,
  onFrame: (state: SqlSourceState) => void,
): number | null {
  const [lastFrameAt, setLastFrameAt] = useState<number | null>(null);
  // The latest callback without re-subscribing per render (the stream is per-source).
  const onFrameRef = useRef(onFrame);
  onFrameRef.current = onFrame;

  useEffect(() => {
    if (!source) return;
    const stream = openBusStream(draftSubject(source), (payload) => {
      const state = parseDraftFrame(payload);
      if (!state) return; // malformed frame — dropped, never crashes the editor
      onFrameRef.current(state);
      setLastFrameAt(Date.now());
    });
    return () => stream?.close();
  }, [source]);

  return lastFrameAt;
}
