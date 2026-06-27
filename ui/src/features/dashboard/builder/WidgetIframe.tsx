// WidgetIframe — the sandboxed render host for a scripted view (plot/d3/template) or an untrusted
// extension widget (widget-builder scope, "No in-process untrusted code"). The author code runs in an
// opaque-origin iframe (`sandbox="allow-scripts"`, NO `allow-same-origin`); this component is the
// PARENT side of the postMessage bridge: it receives `{tool,args}` requests from the frame, re-checks
// the tool against the cell's tool set (defense in depth), forwards through the WidgetBridge to the
// host (which re-checks the cap + workspace), and posts the reply back.
//
// The session token NEVER crosses into the frame — not in srcdoc, not in any reply or watch event. The
// WidgetBridge attaches it server-side. We validate every inbound message came from THIS iframe's
// window before acting (origin is "null" for an opaque-origin frame, so source-identity is the check).

import { useEffect, useRef } from "react";

import type { WidgetBridge } from "./widgetBridge";
import { buildIframeSrcdoc } from "./iframeRuntime";

interface Props {
  engine: "plot" | "d3" | "template";
  code: string;
  /** The cell's tool set = `{source, action}` tools ∩ grant — the bridge enforces it; we re-check too. */
  tools: string[];
  bridge: WidgetBridge;
}

/** Render `code` in a sandboxed iframe, proxying its bridge requests through `bridge`. */
export function WidgetIframe({ engine, code, tools, bridge }: Props) {
  const frameRef = useRef<HTMLIFrameElement>(null);

  useEffect(() => {
    const frame = frameRef.current;
    if (!frame) return;
    const allowed = new Set(tools);
    const unsubs = new Map<string, () => void>();

    const onMessage = async (e: MessageEvent) => {
      // Only act on messages from THIS frame's content window (opaque origin → "null"; source identity
      // is the trustworthy discriminator).
      if (e.source !== frame.contentWindow) return;
      const msg = e.data || {};
      const win = frame.contentWindow;
      if (!win) return;

      if (msg.type === "bridge-call") {
        // Re-check locally (the host re-checks regardless) — a frame asking for a tool outside the set
        // is denied here AND server-side. The reply carries only the result, never a token.
        if (!allowed.has(msg.tool)) {
          win.postMessage({ type: "bridge-reply", id: msg.id, error: `out_of_scope: ${msg.tool}` }, "*");
          return;
        }
        try {
          const result = await bridge.call(msg.tool, msg.args);
          win.postMessage({ type: "bridge-reply", id: msg.id, result }, "*");
        } catch (err) {
          win.postMessage(
            { type: "bridge-reply", id: msg.id, error: err instanceof Error ? err.message : String(err) },
            "*",
          );
        }
      } else if (msg.type === "bridge-watch") {
        if (!allowed.has(msg.tool)) return;
        const unsub = bridge.watch(msg.tool, msg.args ?? {}, (event) => {
          win.postMessage({ type: "watch-event", id: msg.id, event }, "*");
        });
        unsubs.set(msg.id, unsub);
      } else if (msg.type === "bridge-unwatch") {
        unsubs.get(msg.id)?.();
        unsubs.delete(msg.id);
      }
    };

    window.addEventListener("message", onMessage);
    frame.srcdoc = buildIframeSrcdoc({ engine, code, tools });

    return () => {
      window.removeEventListener("message", onMessage);
      // Tear down all live watch streams on unmount/uninstall (stateless eviction).
      unsubs.forEach((u) => u());
      unsubs.clear();
    };
  }, [engine, code, tools, bridge]);

  return (
    <iframe
      ref={frameRef}
      title="scripted-widget"
      sandbox="allow-scripts"
      className="h-full w-full border-0 bg-transparent"
      data-widget-iframe
    />
  );
}
