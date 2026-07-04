// The `[data-call]` click wiring for an in-process rendered template (render-template-inprocess scope,
// Decision 5 — "the data-call wiring reads only data-* attributes"). After `TemplateView` injects the
// sanitized markup via `innerHTML`, it calls this to attach a click handler to every `[data-call]`
// element: the handler reads ONLY `data-call` (the tool name) and `data-args` (a JSON arg blob) — never
// an author-supplied inline handler — and routes the click through the LEASHED bridge (the same
// `makeWidgetBridge` every widget uses: rejected locally if outside `cell.tools`, re-checked at the host
// per call). On resolve it stamps `data-called="ok"`; on a leash/host deny, `data-called="err"`.
//
// This is ported verbatim-in-spirit from the ~18 lines the iframe `template` engine ran inside the
// sandbox (iframeRuntime.ts, now deleted for `template`); the only change is it runs in-process against
// a real DOM element, so the cleanup it returns MUST be called on unmount/re-render (no listener leak).
//
// One responsibility: wire `[data-call]` clicks in a committed subtree to the leashed bridge. Pure DOM
// glue — no React, no data fetching, no token. The XSS-resistance floor is `sanitizeTemplateHtml`; this
// module is belt-and-braces that even a sanitizer miss has no inline-script sink (only data-* is read).

import type { WidgetBridge } from "./widgetBridge";

/** Wire every `[data-call]` element under `root` to a leashed `bridge.call`. Returns a cleanup fn that
 *  detaches what it attached (call on unmount + before re-wiring). Reads ONLY `data-call`/`data-args`.
 *  - `root`    — the committed DOM subtree (TemplateView's `dangerouslySetInnerHTML` container).
 *  - `bridge`  — the leashed WidgetBridge (`makeWidgetBridge(cellTools(cell))`); rejects out-of-leash.
 *  - `tools`   — the cell's tool set; checked BEFORE the call for parity with GenUiView's leash + to
 *                avoid an unnecessary bridge round-trip on an obvious misconfiguration. The host re-checks.
 *  Stamps `data-called="ok"` on success, `"err"` on a leash/host reject or a bad `data-args` JSON blob. */
export function wireTemplateDataCalls(
  root: ParentNode,
  bridge: WidgetBridge,
  tools: readonly string[],
): () => void {
  const buttons = Array.from(root.querySelectorAll<HTMLElement>("[data-call]"));
  const handlers: Array<{ el: HTMLElement; fn: (e: Event) => void }> = [];

  for (const el of buttons) {
    const fn = (e: Event) => {
      e.preventDefault();
      // Read ONLY data-* attributes — never an author inline handler (Decision 5). The tool name is the
      // exact `data-call` string; the args are a JSON blob the author wrote (default {} on a missing/
      // malformed blob — a template authoring bug degrades to a no-arg call, never a crash).
      const tool = el.getAttribute("data-call") ?? "";
      const argsRaw = el.getAttribute("data-args") ?? "{}";
      let args: Record<string, unknown> = {};
      try {
        const parsed = JSON.parse(argsRaw) as unknown;
        if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
          args = parsed as Record<string, unknown>;
        }
      } catch {
        el.setAttribute("data-called", "err");
        return;
      }
      // Leash: never call a tool the cell didn't declare. The bridge re-checks; this is the local gate
      // (parity with GenUiView's `if (!tools.includes(toolName)) return`). The host re-checks the cap +
      // workspace on every call regardless — guard 3 of the widget contract survives the tier change.
      if (!tool || !tools.includes(tool)) {
        el.setAttribute("data-called", "err");
        return;
      }
      void bridge
        .call(tool, args)
        .then(() => el.setAttribute("data-called", "ok"))
        .catch(() => el.setAttribute("data-called", "err"));
    };
    el.addEventListener("click", fn);
    handlers.push({ el, fn });
  }

  // Detach exactly what we attached. The committed subtree is replaced by React on a re-render, so the
  // old nodes are GC'd regardless — but calling this on re-wiring keeps the contract explicit and avoids
  // any double-fire path if a node is ever reused.
  return () => {
    for (const { el, fn } of handlers) el.removeEventListener("click", fn);
  };
}
