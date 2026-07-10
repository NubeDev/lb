// The render-template AI prompt builder (template-prompt slice) — ONE pure function that turns the
// draft's real rows into a copy/paste prompt for any LLM: the template-engine contract (the eval-free
// `{{…}}` grammar, the sanitizer rules, the styling constraints) + a sample of the ACTUAL data, so the
// model designs against the user's real shape and replies with paste-ready template HTML. The rows are
// the user's own query result already rendered on screen — embedding a sample in a prompt the USER
// copies is their call, not an exfiltration path. One responsibility: (rows, sample) → prompt string.

type Row = Record<string, unknown>;

/** How much data to embed: a row count, or "all". */
export type PromptSample = number | "all";

/** The draft's data binding, for provenance in the prompt: which tool/source, and the SQL if the
 *  target carries one (federation / the SQL builder). All fields optional — a flow/series target
 *  still produces a useful prompt without them. */
export interface PromptQuery {
  tool?: string;
  source?: string;
  sql?: string;
}

export function buildTemplatePrompt(rows: Row[], sample: PromptSample, query?: PromptQuery): string {
  const take = sample === "all" ? rows : rows.slice(0, sample);
  const fields = Object.keys(rows[0] ?? {});
  return `Write a render-template widget (HTML only) for the dataset below.

## The template engine (strict — anything else will not run)
- NO JavaScript at all: no <script>, no event handlers, no expressions. The engine is pure
  interpolation over a data object; output is sanitized with DOMPurify before it touches the DOM.
- Bindings: {{rows.length}} · {{latest.FIELD}} (the last row) · {{#each rows}}…{{/each}} iterates the
  rows (single level, no nesting); inside the block {{FIELD}} reads that row and {{.}} is the whole row.
- Unknown paths render as empty text. There is no math, no conditionals, no formatting helpers — if a
  value needs deriving, it must already be a column.
- Optional write buttons: <button data-call="tool.name" data-args='{"k":1}'>…</button> (host-mediated).

## Styling (the widget must look polished and native in the host app)
- INLINE style="" attributes ONLY. A <style> block and its CSS are STRIPPED by the sanitizer, and CSS
  custom properties (--x) won't apply — so put every style on the element, and a bar width is a literal
  width:NN% (from a column), never a var.
- NO <svg> (it is stripped) and no external images — draw icons/marks with CSS (gradients, radial/
  linear backgrounds, borders, box-shadow, border-triangles, small colored divs) or use a unicode glyph.
- Use the host theme tokens so light/dark both work: text hsl(var(--fg))/hsl(var(--muted)), accent
  hsl(var(--accent)), surfaces hsl(var(--panel)), borders hsl(var(--border)); alpha like
  hsl(var(--accent)/0.15). Use tabular-nums for numbers.
- Make it look DESIGNED and BIG, not a plain list: a clear hierarchy with one or two large hero numbers
  (28–44px, font-weight:800), generous padding (16–20px), rounded cards (border-radius:14–20px), a
  gradient or accent-tinted highlight, and comfortable spacing. It fills a whole dashboard tile.
- The root fills the tile: height:100%;box-sizing:border-box; one flex column; inner lists scroll
  (overflow-y:auto); nothing overflows the panel. Keep the whole template under 4 KB.
- For an in-widget show/hide toggle use a native <details><summary>…</summary>…</details> (no JS).

## The query that produces the rows
${
  query?.sql
    ? `${query.source ? `Datasource: ${query.source}` : ""}${query.tool ? ` (via ${query.tool})` : ""}
\`\`\`sql
${query.sql}
\`\`\``
    : query?.tool
      ? `Tool: ${query.tool}${query.source ? ` · source: ${query.source}` : ""} (no SQL — a structured read)`
      : "(no query bound yet)"
}

## The data
Fields: ${fields.join(", ") || "(none yet — run the query first)"}
Total rows at author time: ${rows.length}. Sample (${take.length} row${take.length === 1 ? "" : "s"}) — the template will receive rows of exactly this shape:
${JSON.stringify(take, null, 1)}

## Output
Reply with ONLY the template HTML (no markdown fences, no explanation) — it will be pasted directly
into the inline template editor.`;
}
