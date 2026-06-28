// The LIVE dashboard widget tile (widget-builder scope, "Live feed") — the SSE sibling of WidgetTile.
// Where WidgetTile reads `proof.demo` ONCE (`bridge.call("series.latest")`, state), this one also
// SUBSCRIBES to its motion (`bridge.watch("series.watch")`) and updates on every live sample with no
// reload and no polling (rule 3: state vs motion). The watch rides the shipped series SSE end to end:
//   widget → bridge.watch → openSeriesStream → GET /series/proof.demo/stream → the ws motion subject.
// It reaches only its `[[widget]].scope ∩ grant` (re-checked at the host) — never a token, DB, or fetch.
// On unmount the returned unsubscribe tears the stream down (stateless eviction).

import { useEffect, useState } from "react";

/** The v2 widget bridge — `call` (one-shot read/write) + `watch` (live stream → unsubscribe). */
export interface LiveBridge {
  call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T>;
  watch?: (tool: string, args: Record<string, unknown>, onEvent: (e: unknown) => void) => () => void;
}

interface Sample {
  payload: unknown;
  seq: number;
}

/** A live "latest value" tile for `proof.demo`: backfilled once, then updated per live sample. */
export function WidgetLiveTile({ bridge }: { bridge: LiveBridge }) {
  const [latest, setLatest] = useState<Sample | null>(null);
  const [denied, setDenied] = useState(false);
  const [live, setLive] = useState(false); // true once the SSE subscription is open (honest badge)

  useEffect(() => {
    let cancelled = false;

    // 1) Backfill the current value so the tile isn't empty before the first live sample arrives.
    bridge
      .call<{ sample: Sample | null }>("series.latest", { series: "proof.demo" })
      .then((r) => {
        if (!cancelled) setLatest(r.sample);
      })
      .catch(() => {
        if (!cancelled) setDenied(true);
      });

    // 2) Subscribe to motion. The bridge maps `series.watch` onto the series SSE; each live sample
    //    folds into state. `watch` is optional on the bridge (Tauri/tests with no gateway) — degrade
    //    to the backfilled value when it is absent.
    let unsubscribe: (() => void) | undefined;
    if (typeof bridge.watch === "function") {
      unsubscribe = bridge.watch("series.watch", { series: "proof.demo" }, (event) => {
        const sample = event as Sample | null;
        if (!cancelled && sample && typeof sample === "object" && "payload" in sample) {
          setLatest(sample);
          setLive(true);
        }
      });
    }

    return () => {
      cancelled = true;
      unsubscribe?.();
    };
  }, [bridge]);

  return (
    <div className="flex h-full flex-col p-2" aria-label="proof ping live widget" data-proof-live-widget>
      <div className="flex items-center justify-between">
        <span className="text-xs text-muted">Proof Ping Live · proof.demo</span>
        <span
          className={`text-[10px] uppercase ${live ? "text-emerald-400" : "text-muted"}`}
          data-live={live ? "on" : "off"}
        >
          {live ? "live" : "idle"}
        </span>
      </div>
      <div className="flex flex-1 items-center justify-center">
        {denied ? (
          <span className="text-xs text-red-400">no access</span>
        ) : latest ? (
          <span className="text-2xl font-semibold" aria-label="proof live widget value">
            {String(latest.payload)}
          </span>
        ) : (
          <span className="text-xs text-muted">no value yet</span>
        )}
      </div>
    </div>
  );
}
