// The live series feed over SSE — the dashboard's motion read (mirrors the gateway's
// `GET /series/{series}/stream`). A widget opens this once per distinct series and receives each live
// `Sample` as it is published onto the workspace's series motion subject, which `useSeries` folds into
// its render. State (history) is `series.read`/`series.latest`; this is motion (rule 3) — no polling.
//
// One verb: `openSeriesStream`. It uses the native `EventSource`, so it only runs in a real browser
// against a real gateway — in the Tauri shell and in tests with no gateway URL it returns `null` and
// the widget simply shows its backfilled history (no live updates there, by design).

import type { Sample } from "@/lib/ingest/ingest.types";
import { gatewayUrl } from "@/lib/ipc/http";
import { sessionToken } from "@/lib/session/session.store";

/** A live series stream handle — call `close()` to stop (the hook does this on unmount). */
export interface SeriesStream {
  close: () => void;
}

/** Open the SSE stream for `series`. Returns `null` when no gateway is configured (Tauri / tests) —
 *  the caller then has only the backfilled history, by design. The token rides as a `?token=` query
 *  param (EventSource can't set an Authorization header); the gateway authenticates by it. */
export function openSeriesStream(
  series: string,
  onSample: (sample: Sample) => void,
): SeriesStream | null {
  const base = gatewayUrl();
  if (base === "" && import.meta.env.VITE_GATEWAY_URL === undefined) return null;
  if (typeof EventSource === "undefined") return null;

  const url = `${base}/series/${encodeURIComponent(series)}/stream?token=${encodeURIComponent(
    sessionToken(),
  )}`;
  const es = new EventSource(url);

  es.addEventListener("sample", (e) => {
    try {
      onSample(JSON.parse((e as MessageEvent).data) as Sample);
    } catch {
      // a malformed frame never breaks the stream
    }
  });

  return { close: () => es.close() };
}
