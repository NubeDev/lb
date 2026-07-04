// WidgetIframe — the sandboxed render host for a scripted view (plot/d3) or an untrusted extension
// widget (widget-builder scope, "No in-process untrusted code"). The author code runs in an opaque-origin
// iframe (`sandbox="allow-scripts"`, NO `allow-same-origin`); this component is the PARENT side of the
// postMessage bridge: it receives `{tool,args}` requests from the frame, re-checks the tool against the
// cell's tool set (defense in depth), forwards through the WidgetBridge to the host (which re-checks
// the cap + workspace), and posts the reply back.
//
// The eval-free `template` engine USED to render here too; it was promoted IN-PROCESS (`TemplateView`,
// render-template-inprocess scope) because it runs no author JavaScript. This host now serves ONLY
// `plot`/`d3` (whose snippets `eval` via `new Function` — the sandbox is load-bearing).
//
// The session token NEVER crosses into the frame — not in srcdoc, not in any reply or watch event. The
// WidgetBridge attaches it server-side. We validate every inbound message came from THIS iframe's
// window before acting (origin is "null" for an opaque-origin frame, so source-identity is the check).

import { useEffect, useMemo, useRef } from "react";

import type { WidgetBridge } from "./widgetBridge";
import { buildIframeSrcdoc } from "./iframeRuntime";
import type { TemplateData } from "./templateInterpolate";

interface Props {
  engine: "plot" | "d3";
  code: string;
  /** The cell's tool set = `{source, action}` tools ∩ grant — the bridge enforces it; we re-check too. */
  tools: string[];
  bridge: WidgetBridge;
  /** The panel's source rows (from the parent's `usePanelData`). Seeded into the initial srcdoc AND
   *  posted to the live frame on every change — so a refresh re-renders without rebuilding the iframe. */
  data?: TemplateData;
}

/** Render `code` in a sandboxed iframe, proxying its bridge requests through `bridge` and streaming the
 *  panel's rows in via `postMessage` (no rebuild on data change). */
export function WidgetIframe({ engine, code, tools, bridge, data }: Props) {
  const frameRef = useRef<HTMLIFrameElement>(null);
  // The latest rows, held in a ref so the (engine/code/tools)-keyed build effect reads them WITHOUT
  // re-running on every data tick, and the ready-handshake can reply with the freshest snapshot.
  const dataRef = useRef<TemplateData | undefined>(data);
  const readyRef = useRef(false);
  // A cheap content signature so we only re-post when the rows actually change (usePanelData returns a
  // fresh object each render); avoids re-rendering the frame on unrelated parent re-renders.
  const dataKey = useMemo(() => (data ? JSON.stringify(data) : ""), [data]);

  useEffect(() => {
    const frame = frameRef.current;
    if (!frame) return;
    readyRef.current = false;
    const allowed = new Set(tools);
    const unsubs = new Map<string, () => void>();

    const onMessage = async (e: MessageEvent) => {
      // Only act on messages from THIS frame's content window (opaque origin → "null"; source identity
      // is the trustworthy discriminator).
      if (e.source !== frame.contentWindow) return;
      const msg = e.data || {};
      const win = frame.contentWindow;
      if (!win) return;

      if (msg.type === "frame-ready") {
        // The frame finished its first paint — reply with the freshest rows (covers data that resolved
        // after the srcdoc was built). No token ever crosses; this is data only.
        readyRef.current = true;
        win.postMessage({ type: "render-data", data: dataRef.current ?? { rows: [], latest: null } }, "*");
        return;
      }

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
    frame.srcdoc = buildIframeSrcdoc({ engine, code, tools, data: dataRef.current });

    return () => {
      window.removeEventListener("message", onMessage);
      readyRef.current = false;
      // Tear down all live watch streams on unmount/uninstall (stateless eviction).
      unsubs.forEach((u) => u());
      unsubs.clear();
    };
  }, [engine, code, tools, bridge]);

  // Stream fresh rows to the LIVE frame on every change (post-first-paint). Keyed on the content
  // signature so an identical-rows re-render is a no-op; the frame re-renders with the new data.
  useEffect(() => {
    dataRef.current = data;
    const win = frameRef.current?.contentWindow;
    if (readyRef.current && win) {
      win.postMessage({ type: "render-data", data: data ?? { rows: [], latest: null } }, "*");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `dataKey` is the content signature of `data`
  }, [dataKey]);

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
