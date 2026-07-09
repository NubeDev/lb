// The live series feed over SSE — the dashboard's motion read (mirrors the gateway's
// `GET /series/{series}/stream`). A widget opens this once per distinct series and receives each live
// `Sample` as it is published onto the workspace's series motion subject, which `useSeries` folds into
// its render. State (history) is `series.read`/`series.latest`; this is motion (rule 3) — no polling.
//
// One verb: `openSeriesStream`. It uses the native `EventSource`, so it only runs in a real browser
// against a real gateway — in the Tauri shell and in tests with no gateway URL it returns `null` and
// the widget simply shows its backfilled history (no live updates there, by design).

import { eventHub, liveStreamAvailable } from "@/lib/events/hub";
import type { Sample } from "@/lib/ingest/ingest.types";

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
  if (!liveStreamAvailable()) return null;
  // Delegates to the shared event hub (unified-event-stream scope): the `series:{series}` subject rides
  // the one multiplexed connection. N cells on one series share ONE server subscription (the hub dedupes),
  // closing the deferred "one EventSource per series, fanned to N cells" follow-up. The `event: sample`
  // frame is byte-identical to the dedicated route's, so the fold below is unchanged.
  const unsubscribe = eventHub.subscribeSubject(`series:${series}`, (frame) => {
    if (frame.event !== "sample") return;
    try {
      onSample(JSON.parse(frame.data) as Sample);
    } catch {
      // a malformed frame never breaks the stream
    }
  });
  return { close: unsubscribe };
}
