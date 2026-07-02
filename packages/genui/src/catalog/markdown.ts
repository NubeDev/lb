// `renderMarkdown` — a SAFE minimal markdown → React-element renderer (promotion-checklist items 1+2).
// It NEVER uses `dangerouslySetInnerHTML` and NEVER passes raw HTML through: any `<...>` in the source
// is treated as literal text (React escapes it). Supported: ATX headings (#..######), unordered/ordered
// lists, blank-line paragraphs, and inline **bold**, *italic*, `code`, [text](href) links and hard line
// breaks. A link becomes an <a href> ONLY when its scheme is http/https/mailto; otherwise the link text
// renders as plain text (no href). No prop is ever evaluated as code.

import { createElement, type ReactNode } from "react";

const SAFE_SCHEME = /^(https?:|mailto:)/i;

/** Return the href only if its scheme is on the whitelist, else null (link degrades to plain text). */
function safeHref(raw: string): string | null {
  const href = raw.trim();
  // Relative/hash links have no scheme → reject (we can't know the origin here; keep it strict).
  if (SAFE_SCHEME.test(href)) return href;
  return null;
}

// ── inline tokenizer ──────────────────────────────────────────────────────────────────────────────
// Matches, in order: `code`, **bold**, *italic*, [text](href). Everything else is literal text.
const INLINE = /(`[^`]+`)|(\*\*[^*]+\*\*)|(\*[^*]+\*)|(\[[^\]]*\]\([^)]*\))/g;

function renderInline(text: string, keyBase: string): ReactNode[] {
  const out: ReactNode[] = [];
  let last = 0;
  let m: RegExpExecArray | null;
  let i = 0;
  INLINE.lastIndex = 0;
  while ((m = INLINE.exec(text)) !== null) {
    if (m.index > last) out.push(text.slice(last, m.index));
    const tok = m[0];
    const key = `${keyBase}-${i++}`;
    if (tok.startsWith("`")) {
      out.push(createElement("code", { key }, tok.slice(1, -1)));
    } else if (tok.startsWith("**")) {
      out.push(createElement("strong", { key }, tok.slice(2, -2)));
    } else if (tok.startsWith("*")) {
      out.push(createElement("em", { key }, tok.slice(1, -1)));
    } else {
      // [label](href)
      const close = tok.indexOf("](");
      const label = tok.slice(1, close);
      const href = tok.slice(close + 2, -1);
      const safe = safeHref(href);
      if (safe) out.push(createElement("a", { key, href: safe, rel: "noopener noreferrer", target: "_blank" }, label));
      else out.push(label);
    }
    last = m.index + tok.length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

/** Split a paragraph's raw text into inline nodes with hard line breaks between physical lines. */
function renderParagraphLines(lines: string[], keyBase: string): ReactNode[] {
  const out: ReactNode[] = [];
  lines.forEach((line, idx) => {
    if (idx > 0) out.push(createElement("br", { key: `${keyBase}-br-${idx}` }));
    out.push(...renderInline(line, `${keyBase}-l${idx}`));
  });
  return out;
}

// ── block parser ──────────────────────────────────────────────────────────────────────────────────
const HEADING = /^(#{1,6})\s+(.*)$/;
const UL_ITEM = /^\s*[-*+]\s+(.*)$/;
const OL_ITEM = /^\s*\d+[.)]\s+(.*)$/;

export function renderMarkdown(source: string): ReactNode[] {
  const src = typeof source === "string" ? source : "";
  const lines = src.replace(/\r\n?/g, "\n").split("\n");
  const blocks: ReactNode[] = [];
  let i = 0;
  let key = 0;

  while (i < lines.length) {
    const line = lines[i];

    // blank line → skip
    if (line.trim() === "") {
      i++;
      continue;
    }

    // heading
    const h = HEADING.exec(line);
    if (h) {
      const level = h[1].length;
      blocks.push(createElement(`h${level}`, { key: key++ }, renderInline(h[2], `h${key}`)));
      i++;
      continue;
    }

    // list (unordered or ordered) — a run of consecutive matching items
    const isUl = UL_ITEM.test(line);
    const isOl = !isUl && OL_ITEM.test(line);
    if (isUl || isOl) {
      const re = isUl ? UL_ITEM : OL_ITEM;
      const items: ReactNode[] = [];
      let j = 0;
      while (i < lines.length) {
        const mm = re.exec(lines[i]);
        if (!mm) break;
        items.push(createElement("li", { key: j }, renderInline(mm[1], `li${key}-${j}`)));
        j++;
        i++;
      }
      blocks.push(createElement(isUl ? "ul" : "ol", { key: key++ }, items));
      continue;
    }

    // paragraph — gather until blank line or a block starter
    const para: string[] = [];
    while (i < lines.length && lines[i].trim() !== "" && !HEADING.test(lines[i]) && !UL_ITEM.test(lines[i]) && !OL_ITEM.test(lines[i])) {
      para.push(lines[i]);
      i++;
    }
    blocks.push(createElement("p", { key: key++ }, renderParagraphLines(para, `p${key}`)));
  }

  return blocks;
}
