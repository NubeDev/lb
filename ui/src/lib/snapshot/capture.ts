// Client-side block snapshot capture (reports scope, "Panels export as client snapshots"). The browser
// is already rendering every widget LIVE under the viewer's caps; at export time it captures each panel
// block's rendered DOM to a PNG and sends the snapshots with the export request. The server NEVER
// fetches widget data for export — the PDF can only ever contain what the exporting user could see.
//
// Two paths, in order:
//   1. ECharts fast path — if the element hosts an ECharts instance, its native `getDataURL` is the
//      crispest capture. (Most shipped charts are Recharts/SVG today; this stays as the fast path for
//      any ECharts-backed extension widget — NOTE below.)
//   2. `html-to-image` `toPng` fallback — snapshots the live DOM subtree (SVG, canvas, HTML) to a PNG
//      data-URI. This is the common path for the shipped Recharts widgets.
//
// One responsibility: turn a rendered element into a base64 PNG data-URI. Extension widgets in
// sandboxed tiers may not be capturable → the caller renders a titled placeholder rather than failing
// the whole export (per scope "Snapshot fidelity").

import { toPng } from "html-to-image";

/** An ECharts instance exposes `getDataURL`; we duck-type it off the element without importing echarts
 *  (it isn't a dep here — this is the opportunistic fast path for widgets that do use it). */
interface EChartsLike {
  getDataURL(opts?: { type?: string; pixelRatio?: number; backgroundColor?: string }): string;
}

/** ECharts stashes its instance on the container via `echarts.getInstanceByDom`; some builds also hang
 *  `__ecInstance__` off the DOM node. We probe the node subtree for either without a hard dep. */
function echartsInstance(el: HTMLElement): EChartsLike | null {
  const g = globalThis as unknown as {
    echarts?: { getInstanceByDom(dom: HTMLElement): EChartsLike | undefined };
  };
  const nodes = [el, ...Array.from(el.querySelectorAll<HTMLElement>("[_echarts_instance_]"))];
  for (const n of nodes) {
    const direct = (n as unknown as { __ecInstance__?: EChartsLike }).__ecInstance__;
    if (direct?.getDataURL) return direct;
    const viaLib = g.echarts?.getInstanceByDom(n);
    if (viaLib?.getDataURL) return viaLib;
  }
  return null;
}

/** Capture `el` to a base64 PNG data-URI. ECharts fast path if present, else `html-to-image`. Rejects
 *  if capture fails (the caller decides whether to substitute a placeholder). */
export async function captureBlock(el: HTMLElement): Promise<string> {
  const ec = echartsInstance(el);
  if (ec) {
    return ec.getDataURL({ type: "png", pixelRatio: 2, backgroundColor: "#ffffff" });
  }
  return toPng(el, { pixelRatio: 2, backgroundColor: "#ffffff", cacheBust: true });
}
