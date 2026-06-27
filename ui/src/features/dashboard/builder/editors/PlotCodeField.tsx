// The Observable Plot / D3 snippet editor (widget-builder Slice B) — ported from rubix-cube's
// `PlotCodeField.tsx`, with its zustand-editor-store data layer swapped for the builder's
// `value`/`onChange` props. It carries a `DEFAULT_PLOT_CODE` and a bindings hint so a fresh Plot/D3
// widget starts from a working snippet.
//
// IMPORTANT — the snippet convention is lazybones's iframe runtime, NOT rubix-cube's. The shipped
// runtime calls `spec(bridge, root, engineMod)`, so a snippet is `async (bridge, el, Plot) => { … }`:
// it fetches its own rows via `bridge.call(...)` (the host-mediated tool surface) and mounts an element
// into `el`. (rubix-cube handed `data` in directly; here the snippet pulls data through the bridge —
// the v2 token-less, grant-leashed path.) The code runs ONLY in the sandboxed iframe.

import { CodeEditor } from "./CodeEditor";

/** A working default Plot snippet for a fresh `plot` widget — fetches a series via the bridge and
 *  mounts a line chart. `engine` is the CDN-loaded Observable Plot module the runtime injects. */
export const DEFAULT_PLOT_CODE = `async (bridge, el, Plot) => {
  // Pull rows through the host-mediated bridge (any granted read tool — e.g. store.query or series.read).
  const { rows } = await bridge.call("store.query", { sql: "SELECT seq, payload FROM series ORDER BY seq LIMIT 100" });
  const fig = Plot.plot({
    marginLeft: 50,
    marks: [Plot.lineY(rows, { x: "seq", y: "payload", stroke: "steelblue" }), Plot.ruleY([0])],
  });
  el.replaceChildren(fig);
}`;

/** A working default D3 snippet for a fresh `d3` widget (the `engine` arg is the CDN-loaded d3 module). */
export const DEFAULT_D3_CODE = `async (bridge, el, d3) => {
  const { rows } = await bridge.call("store.query", { sql: "SELECT seq, payload FROM series ORDER BY seq LIMIT 100" });
  el.textContent = JSON.stringify(rows.slice(0, 10));
}`;

interface Props {
  /** "plot" or "d3" — picks the default snippet + the bindings hint. */
  engine: "plot" | "d3";
  /** The current snippet string (inline `cell.options.code`). */
  value: string;
  /** Called with the new snippet on every edit. */
  onChange: (value: string) => void;
}

/** The Plot/D3 code editor — a `CodeEditor` plus a bindings hint. The default snippet is seeded by the
 *  builder when the author first picks the view (so the editor stays a pure controlled component). */
export function PlotCodeField({ engine, value, onChange }: Props) {
  return (
    <div className="mt-2 grid gap-1">
      <CodeEditor
        value={value}
        onChange={onChange}
        ariaLabel={`${engine} code`}
        height="160px"
      />
      <p className="text-[10px] text-muted">
        Signature: <span className="font-mono">async (bridge, el, {engine === "plot" ? "Plot" : "d3"})</span>.
        Fetch rows via <span className="font-mono">bridge.call(tool, args)</span> (a granted read tool —
        e.g. <span className="font-mono">store.query</span>); mount an element into{" "}
        <span className="font-mono">el</span>.
      </p>
    </div>
  );
}
