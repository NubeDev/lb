// The dashboard WIDGET tile this extension contributes (widget-builder scope, follow-up — the model
// for an extension-shipped `[[widget]]`). It is the page's little sibling: the SAME federated remote,
// a SECOND named export (`mountWidget`), rendering a compact tile that reads the latest of a demo
// series through the host-mediated v2 bridge (`bridge.call`/`bridge.watch`). It reaches only its
// `[[widget]].scope ∩ grant` (re-checked at the host); it never sees a token, DB, or fetch.

import { useEffect, useState } from "react";

/** The v2 widget bridge the shell passes — `call` (read/write a granted tool) + `watch` (stream). */
export interface WidgetBridge {
  call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T>;
  watch?: (tool: string, args: Record<string, unknown>, onEvent: (e: unknown) => void) => () => void;
}

interface Sample {
  payload: unknown;
  seq: number;
}

/** A compact "latest value" tile for the `proof.demo` series. Honest empty/denied states, no fake. */
export function WidgetTile({ bridge }: { bridge: WidgetBridge }) {
  const [latest, setLatest] = useState<Sample | null>(null);
  const [denied, setDenied] = useState(false);

  useEffect(() => {
    let cancelled = false;
    bridge
      .call<{ sample: Sample | null }>("series.latest", { series: "proof.demo" })
      .then((r) => {
        if (!cancelled) setLatest(r.sample);
      })
      .catch(() => {
        if (!cancelled) setDenied(true);
      });
    return () => {
      cancelled = true;
    };
  }, [bridge]);

  return (
    <div className="flex h-full flex-col p-2" aria-label="proof ping widget" data-proof-widget>
      <span className="text-xs text-muted">Proof Ping · proof.demo</span>
      <div className="flex flex-1 items-center justify-center">
        {denied ? (
          <span className="text-xs text-red-400">no access</span>
        ) : latest ? (
          <span className="text-2xl font-semibold" aria-label="proof widget value">
            {String(latest.payload)}
          </span>
        ) : (
          <span className="text-xs text-muted">no value yet</span>
        )}
      </div>
    </div>
  );
}
