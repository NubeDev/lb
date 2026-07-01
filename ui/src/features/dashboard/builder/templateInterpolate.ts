// The eval-free `template`-engine interpolator (widget-builder scope, "Scripted views"). ONE pure,
// self-contained function that turns an author HTML/JSX snippet + the panel's source rows into a final
// HTML string: `{{path}}` scalar binding + `{{#each list}}…{{/each}}` iteration, with every interpolated
// DATA value HTML-escaped (the template markup is author-written and trusted for structure; the data is
// the viewer's grant and is escaped so a row value can't inject markup). NO `new Function`, no eval — the
// same tiny interpreter the sandboxed iframe runs (it is embedded into the frame runtime via
// `.toString()`, so this file is the SINGLE source of truth and is unit-tested directly).
//
// Kept CLOSURE-FREE on purpose: `interpolateTemplate` references nothing in module scope, so
// `interpolateTemplate.toString()` yields a complete, runnable definition when embedded in the frame.

/** The data context a template binds against — the `SourceState` a panel's `usePanelData` produced,
 *  narrowed to what a template reads. `{{rows.length}}` / `{{#each rows}}` iterate `rows`; `{{latest.x}}`
 *  reads the last row; `loading`/`denied` let a template show its own empty/denied note. */
export interface TemplateData {
  rows: Array<Record<string, unknown>>;
  latest?: unknown;
  loading?: boolean;
  denied?: boolean;
}

/** Interpolate `code` against `data` — `{{path}}` and `{{#each list}}…{{/each}}`, data values escaped.
 *  Unknown paths render empty (never `undefined`/crash). `{{#each}}` over a non-array renders nothing.
 *  Inside an `each` block the context IS the item, so `{{field}}` reads the row and `{{.}}` the whole row.
 *  Single-level `each` only (a nested `each` is not supported — the first `{{/each}}` closes the block). */
export function interpolateTemplate(code: string, data: TemplateData): string {
  function escapeHtml(s: string): string {
    return s
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }
  // Resolve a dotted path against a context object. `.`/`this` → the context itself. A missing segment
  // short-circuits to undefined (rendered as empty), never throws.
  function resolve(ctx: unknown, path: string): unknown {
    const key = path.trim();
    if (key === "." || key === "this") return ctx;
    let v: unknown = ctx;
    for (const part of key.split(".")) {
      if (v == null || typeof v !== "object") return undefined;
      v = (v as Record<string, unknown>)[part];
    }
    return v;
  }
  function render(tpl: string, ctx: unknown): string {
    // 1. Expand `{{#each list}}…{{/each}}` (non-greedy: the FIRST `{{/each}}` closes it). Each item is
    //    rendered with the item as its context, so `{{field}}` reads the row directly.
    const expanded = tpl.replace(
      /\{\{#each\s+([^}]+?)\}\}([\s\S]*?)\{\{\/each\}\}/g,
      (_m, expr: string, body: string) => {
        const list = resolve(ctx, expr);
        if (!Array.isArray(list)) return "";
        return list.map((item) => render(body, item)).join("");
      },
    );
    // 2. Substitute the remaining `{{ path }}` scalars, escaping every value. `[^#/]` first char skips any
    //    stray block token; an object value renders as compact JSON (honest, not `[object Object]`).
    return expanded.replace(/\{\{\s*([^#/][^}]*?)\s*\}\}/g, (_m, expr: string) => {
      const v = resolve(ctx, expr);
      if (v == null) return "";
      return escapeHtml(typeof v === "object" ? JSON.stringify(v) : String(v));
    });
  }
  return render(code, data);
}
