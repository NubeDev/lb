// The telemetry live tail over SSE — the console's motion read (mirrors the gateway's
// `GET /telemetry/stream`). The console opens this once when live-tail is on and folds each arriving
// row into the list. State (recent history) is `telemetry.query`; this is motion (rule 3) — no polling.
//
// One verb: `openTelemetryStream`. It uses the native `EventSource`, so it runs only in a real browser
// against a real gateway — in the Tauri shell and in tests with no gateway URL it returns `null` and
// the console shows only the snapshot (no live updates there, by design). The token rides as a
// `?token=` query param (EventSource can't set headers); the gateway authenticates by it, checks
// `mcp:telemetry.read:call` (403 before any body), and the bus subject is ws-walled.

import { gatewayUrl } from "@/lib/ipc/http";
import { sessionToken } from "@/lib/session/session.store";
import { normalizeRow } from "./telemetry.api";
import type { TelemetryRow } from "./telemetry.types";

/** A live telemetry stream handle — call `close()` to stop (the hook does this on unmount/toggle). */
export interface TelemetryStream {
  close: () => void;
}

/** Open the SSE tail. Returns `null` when no gateway is configured (Tauri / tests) — the caller then
 *  has only the snapshot. Emits the catch-up snapshot as `event: snapshot` frames first, then each
 *  live row as `event: telemetry`; both are folded through `onRow`. */
export function openTelemetryStream(
  onRow: (row: TelemetryRow) => void,
): TelemetryStream | null {
  const base = gatewayUrl();
  if (base === "" && import.meta.env.VITE_GATEWAY_URL === undefined) return null;
  if (typeof EventSource === "undefined") return null;

  const url = `${base}/telemetry/stream?token=${encodeURIComponent(sessionToken())}`;
  const es = new EventSource(url);

  const fold = (e: Event) => {
    try {
      onRow(normalizeRow(JSON.parse((e as MessageEvent).data) as Record<string, unknown>));
    } catch {
      // a malformed frame never breaks the stream
    }
  };
  es.addEventListener("snapshot", fold);
  es.addEventListener("telemetry", fold);

  return { close: () => es.close() };
}
