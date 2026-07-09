// JsonTreeView — a compact, collapsible JSON tree for the debug panel (debug-node-scope). Reuses the
// shipped `@microlink/react-json-view` (already a dep, same one `features/channel/MarkdownView` uses
// for ```json blocks```) so a debug `value` renders as the same interactive tree the rest of the
// workbench speaks. One responsibility (FILE-LAYOUT): a JSON value in, a styled tree out — no
// data/effects. Primitives are wrapped so the tree always has a container root.

import ReactJson from "@microlink/react-json-view";

interface Props {
  /** The parsed JSON value to render. */
  value: unknown;
  /** Start collapsed (the panel collapses long values; the tree honours by starting collapsed too). */
  collapsed?: boolean;
}

export function JsonTreeView({ value, collapsed }: Props) {
  const root = value !== null && typeof value === "object" ? (value as object) : { value };
  return (
    <div aria-label="debug json tree" className="rounded-md border border-border bg-panel-2/40 p-1">
      <ReactJson
        src={root}
        name={false}
        theme={JSON_THEME}
        iconStyle="triangle"
        indentWidth={2}
        collapsed={collapsed ?? 1}
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

// The base-16 theme mapped onto the workbench's tokens — the same mapping as
// `features/channel/MarkdownView`'s `JSON_THEME`, inlined so this renderer stands alone (no cross-
// feature import for a constant). Only chrome (keys, punctuation, glyphs) is tinted; values read
// faithfully.
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
