// The sandboxed-iframe runtime — the HTML+script that renders a SCRIPTED view (Plot/D3/JSX `template`)
// or an UNTRUSTED extension widget inside an opaque-origin iframe (widget-builder scope, "No
// in-process untrusted code"). This string is injected via `srcdoc` into an iframe whose `sandbox`
// is `allow-scripts` ONLY — NO `allow-same-origin`, so the frame runs in a unique opaque origin: it
// cannot read the parent's cookies, localStorage, or the session token, and its `postMessage` `origin`
// is `"null"`.
//
// The frame reaches data ONLY by posting `{type:"bridge-call"|"bridge-watch", id, tool, args}` to the
// parent; the parent (WidgetIframe) re-checks the tool against cell.tools ∩ grant and forwards to the
// host (which re-checks the cap + workspace). The token NEVER crosses this boundary — not in a request,
// reply, or watch event. The frame validates `event.source === window.parent` on every inbound message.
//
// The engines (Plot/D3) are loaded from a pinned CDN INSIDE the sandbox; if offline they degrade to a
// plain-rows render. The JSX `template` engine is the eval-free {@link interpolateTemplate} interpreter
// over the panel's source rows (embedded via `.toString()` — ONE source of truth, unit-tested). The rows
// arrive from the parent's ONE data hook (`usePanelData`) as an initial config value AND as live
// `render-data` postMessages, so a template/plot/d3 view refreshes without rebuilding the frame. All
// author code executes here, never in the shell.

import { interpolateTemplate, type TemplateData } from "./templateInterpolate";

/** Build the `srcdoc` HTML for a scripted/iframe widget. `engine` selects the renderer; `code` is the
 *  author snippet (Plot/D3/JSX); `tools` is the cell's tool set (the frame shows which writes it may
 *  call, but the parent is the real gate); `data` is the panel's initial rows (the parent posts fresh
 *  rows as `render-data` on every refresh). The CSP forbids inline-script injection beyond our own
 *  bootstrap and pins the engine origins. */
export function buildIframeSrcdoc(opts: {
  engine: "plot" | "d3" | "template";
  code: string;
  tools: string[];
  data?: TemplateData;
}): string {
  const payload = JSON.stringify({
    engine: opts.engine,
    code: opts.code,
    tools: opts.tools,
    data: opts.data ?? { rows: [], latest: null },
  }).replace(/</g, "\\u003c");

  // CSP: default-src 'none' (nothing by default); scripts only from self + the pinned engine CDN;
  // styles inline allowed for the rendered output. No connect-src/img beyond data: — the frame reaches
  // the network ONLY through the parent bridge, never directly.
  return `<!doctype html>
<html>
<head>
<meta charset="utf-8" />
<meta http-equiv="Content-Security-Policy"
  content="default-src 'none'; script-src 'unsafe-inline' https://cdn.jsdelivr.net; style-src 'unsafe-inline'; img-src data:; connect-src 'none'" />
<style>
  body { margin: 0; font: 13px system-ui, sans-serif; color: #e5e7eb; background: transparent; }
  #root { padding: 8px; }
  .err { color: #f87171; white-space: pre-wrap; }
  button { cursor: pointer; }
</style>
</head>
<body>
<div id="root"></div>
<script id="cfg" type="application/json">${payload}</script>
<script type="module">
// The eval-free template interpreter, embedded from the SAME unit-tested shell export (it is closure-free
// so its serialized form is complete) — the frame and the tests share one definition.
const interpolateTemplate = ${interpolateTemplate.toString()};
${IFRAME_BOOTSTRAP}
</script>
</body>
</html>`;
}

// The bootstrap script that runs INSIDE the sandbox. It:
//  1. Sets up the parent bridge over postMessage (call/watch), never touching any token.
//  2. Reads the config, fetches the rows via a `series.read`-style source the parent seeded (or the
//     widget calls bridge.call itself), and renders by engine.
//  3. Validates every inbound message's source is the parent.
const IFRAME_BOOTSTRAP = String.raw`
const PARENT = window.parent;
const pending = new Map();
const watchers = new Map();
let seq = 0;

// Only accept messages from the parent frame; ignore everything else (opaque-origin discipline).
window.addEventListener("message", (e) => {
  if (e.source !== PARENT) return;
  const msg = e.data || {};
  if (msg.type === "bridge-reply") {
    const p = pending.get(msg.id);
    if (p) { pending.delete(msg.id); msg.error ? p.reject(new Error(msg.error)) : p.resolve(msg.result); }
  } else if (msg.type === "watch-event") {
    const cb = watchers.get(msg.id);
    if (cb) cb(msg.event);
  } else if (msg.type === "render-data") {
    // Fresh rows from the parent's data hook (initial handshake reply OR a live refresh) — re-render with
    // them. Cheap for a template (interpolate + innerHTML); re-runs the plot/d3 snippet with new rows.
    cfg.data = msg.data || { rows: [], latest: null };
    render();
  }
});

// The bridge the author code sees — call(tool,args) and watch(tool,args,onEvent). NO token is ever
// present; the parent injects it server-side. A call posts a request and resolves on the matching reply.
const bridge = {
  call(tool, args) {
    const id = "c" + (++seq);
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
      PARENT.postMessage({ type: "bridge-call", id, tool, args: args || {} }, "*");
    });
  },
  watch(tool, args, onEvent) {
    const id = "w" + (++seq);
    watchers.set(id, onEvent);
    PARENT.postMessage({ type: "bridge-watch", id, tool, args: args || {} }, "*");
    return () => { watchers.delete(id); PARENT.postMessage({ type: "bridge-unwatch", id }, "*"); };
  },
};

const cfg = JSON.parse(document.getElementById("cfg").textContent);
const root = document.getElementById("root");

function showError(e) { root.innerHTML = '<div class="err">' + String(e && e.message || e) + '</div>'; }

// Render by engine. Each gets the rows (the author calls bridge.call to read them) + the bridge so a
// scripted view may WRITE (a granted write tool) — the iframe sandbox + grant + host re-check are the
// three guards (widget-builder scope, "Scripted views ... may write").
async function render() {
  try {
    if (cfg.engine === "template") {
      // The 'template' engine: eval-free {{path}} + {{#each}} interpolation over the panel's source rows
      // (cfg.data = {rows, latest, ...} from the parent's usePanelData). Data values are escaped by the
      // interpreter; the author markup is set as innerHTML. [data-call] elements are wired to bridge
      // WRITES (the friendly "Defrost" button) — the only place a template touches a tool.
      window.__bridge = bridge;
      root.innerHTML = interpolateTemplate(cfg.code, cfg.data || { rows: [], latest: null });
      // Wire any [data-call] element to a write through the bridge (re-wired each render; innerHTML reset).
      root.querySelectorAll("[data-call]").forEach((el) => {
        el.addEventListener("click", async () => {
          try {
            const tool = el.getAttribute("data-call");
            const args = JSON.parse(el.getAttribute("data-args") || "{}");
            await bridge.call(tool, args);
            el.setAttribute("data-called", "ok");
          } catch (err) { el.setAttribute("data-called", "err"); showError(err); }
        });
      });
    } else {
      // plot / d3: load the pinned engine, hand it (bridge, the root el, the engine module, the panel
      // rows). The author may read rows straight from the 'rows' arg (the parent's usePanelData) OR call
      // bridge itself. If the CDN is unreachable (offline), degrade to a JSON dump — never a fake chart.
      const spec = new Function("bridge", "el", "engine", "rows", cfg.code);
      let engineMod = null;
      try {
        engineMod =
          cfg.engine === "plot"
            ? await import("https://cdn.jsdelivr.net/npm/@observablehq/plot@0.6/+esm")
            : await import("https://cdn.jsdelivr.net/npm/d3@7/+esm");
      } catch { engineMod = null; }
      await spec(bridge, root, engineMod, (cfg.data && cfg.data.rows) || []);
    }
    PARENT.postMessage({ type: "rendered" }, "*");
  } catch (e) { showError(e); PARENT.postMessage({ type: "render-error", error: String(e && e.message || e) }, "*"); }
}

// First paint from the config's initial rows, then tell the parent we're live so it replies with the
// freshest rows (covers the race where data resolved after the frame's srcdoc was built).
render();
PARENT.postMessage({ type: "frame-ready" }, "*");
`;
