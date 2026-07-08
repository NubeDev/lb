// MarkdownView — the agent's markdown renderer (channels-agent scope). Turns an LLM's markdown answer
// into React nodes via `react-markdown` + `remark-gfm`. XSS-safe by construction: react-markdown builds
// vnodes (no raw HTML reaches the DOM unless `rehype-raw` is added — we don't), so `dompurify` isn't
// needed here. ```json fenced blocks``` render as the workbench's INTERACTIVE JSON tree
// (`@microlink/react-json-view` — already a dep, used by `features/rules/JsonTree`); other fenced
// blocks render as a styled `<pre><code>`; inline code styles inline. One responsibility (FILE-LAYOUT):
// markdown in, React out — no data/effects.

import type { ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import ReactJson from "@microlink/react-json-view";

interface Props {
  /** The markdown source — typically `payload.answer` or a streamed `feed.text` delta accumulation. */
  children: string;
}

/** Parse `text` as JSON, returning the value or `null` on failure. A bare primitive is wrapped so the
 *  tree has a container root (matches JsonTree's handling). */
function tryParseJson(text: string): unknown | null {
  const trimmed = text.trim();
  if (!trimmed) return null;
  try {
    const value: unknown = JSON.parse(trimmed);
    return value;
  } catch {
    return null;
  }
}

/** Render a parsed JSON value as the interactive tree. Primitives get wrapped so the tree has a root. */
function JsonBlock({ value }: { value: unknown }) {
  const root = value !== null && typeof value === "object" ? (value as object) : { value };
  return (
    <div
      aria-label="json block"
      className="my-2 rounded-md border border-border bg-panel-2/40 p-2"
    >
      <ReactJson
        src={root}
        name={false}
        theme={JSON_THEME}
        iconStyle="triangle"
        indentWidth={2}
        collapsed={false}
        groupArraysAfterLength={100}
        collapseStringsAfterLength={200}
        displayDataTypes={false}
        displayObjectSize
        enableClipboard
        quotesOnKeys={false}
        style={{ backgroundColor: "transparent", fontFamily: "var(--font-mono, monospace)" }}
      />
    </div>
  );
}

export function MarkdownView({ children }: Props) {
  return (
    <div className="markdown-view break-words text-sm leading-6 text-fg [&>*+*]:mt-2 [&_a]:text-accent [&_a]:underline [&_blockquote]:border-l-2 [&_blockquote]:border-border [&_blockquote]:pl-3 [&_blockquote]:text-muted [&_h1]:mt-3 [&_h1]:text-base [&_h1]:font-semibold [&_h2]:mt-3 [&_h2]:text-sm [&_h2]:font-semibold [&_h3]:mt-2 [&_h3]:font-semibold [&_hr]:my-3 [&_hr]:border-border [&_li]:my-0.5 [&_ol]:list-decimal [&_ol]:pl-5 [&_ol>li>input]:mr-1 [&_table]:w-full [&_table]:border-collapse [&_td]:border [&_td]:border-border [&_td]:px-2 [&_td]:py-1 [&_th]:border [&_th]:border-border [&_th]:px-2 [&_th]:py-1 [&_th]:text-left [&_th]:font-medium [&_ul]:list-disc [&_ul]:pl-5 [&_ul>li>input]:mr-1">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        // Unwrap <pre> so the JSON tree (a <div>) never nests inside a <pre> (invalid markup). Each
        // fenced block's container is owned by the `code` renderer below.
        components={{
          pre: ({ children: c }: { children?: ReactNode }) => <>{c}</>,
          code: ({ className, children: code }: { className?: string; children?: ReactNode }) => {
            const match = /language-(\w+)/.exec(className || "");
            const raw = String(code ?? "").replace(/\n$/, "");
            const isBlock = !!match || raw.includes("\n");

            // JSON tree (interactive) — only when the block parses to a value.
            if (match?.[1] === "json") {
              const parsed = tryParseJson(raw);
              if (parsed !== null) return <JsonBlock value={parsed} />;
            }

            if (isBlock) {
              return (
                <pre className="my-2 overflow-x-auto rounded-md border border-border bg-panel-2/60 p-2 text-xs leading-5">
                  <code className="font-mono text-fg/90">{raw}</code>
                </pre>
              );
            }
            // Inline code.
            return (
              <code className="rounded-sm bg-panel-2/80 px-1 py-0.5 font-mono text-[0.85em] text-accent">
                {code}
              </code>
            );
          },
          a: ({ children: c, href }: { children?: ReactNode; href?: string }) => (
            <a href={href} target="_blank" rel="noopener noreferrer">
              {c}
            </a>
          ),
        }}
      >
        {children}
      </ReactMarkdown>
    </div>
  );
}

// The base-16 theme for the embedded JSON tree, mapped onto the workbench's dark-surface tokens — the
// same mapping as `features/rules/JsonTree`'s `RULES_JSON_THEME`, inlined so this renderer stands alone
// (no cross-feature import). Only chrome (keys, punctuation, glyphs) is tinted; values read faithfully.
const JSON_THEME = {
  base00: "transparent",
  base01: "hsl(var(--border))",
  base02: "hsl(var(--border))",
  base03: "hsl(var(--muted))",
  base04: "hsl(var(--muted))",
  base05: "hsl(var(--fg))",
  base06: "hsl(var(--fg))",
  base07: "hsl(var(--fg))",
  base08: "hsl(var(--muted))",
  base09: "hsl(var(--accent))",
  base0A: "hsl(var(--accent))",
  base0B: "hsl(var(--accent))",
  base0C: "hsl(var(--accent))",
  base0D: "hsl(var(--muted))",
  base0E: "hsl(var(--muted))",
  base0F: "hsl(var(--accent))",
} as const;
