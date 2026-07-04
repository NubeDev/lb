// JsonTree — the shared interactive JSON renderer for the workbench result region (rules-editor-ux
// scope). A collapsible, syntax-highlighted tree over any value, deep-parsing embedded JSON strings
// first (`parseEmbeddedJson`) so a nested payload — a `channel.history` row's JSON-string `body` — reads
// as a humane expandable object rather than an escaped blob. Used by both `JsonView` (the whole
// `RunResult`) and `ScalarCard` (a structured scalar value). Faithful, never abridged. One component per
// file (FILE-LAYOUT).

import ReactJson from "@microlink/react-json-view";

import { parseEmbeddedJson } from "./parseEmbeddedJson";

interface JsonTreeProps {
  /** Any value — deep-parsed then rendered. A bare scalar is wrapped so the tree has a container root. */
  src: unknown;
}

export function JsonTree({ src }: JsonTreeProps) {
  const parsed = parseEmbeddedJson(src);
  const root =
    parsed !== null && typeof parsed === "object" ? (parsed as object) : { value: parsed };

  return (
    <ReactJson
      src={root}
      name={false}
      theme={RULES_JSON_THEME}
      iconStyle="triangle"
      indentWidth={2}
      // Expanded by default (faithful, never abridged — the old <pre> showed everything); deep arrays
      // group after 100 and every node stays click-collapsible for a large channel history.
      collapsed={false}
      groupArraysAfterLength={100}
      collapseStringsAfterLength={200}
      displayDataTypes={false}
      displayObjectSize
      enableClipboard
      quotesOnKeys={false}
      style={{ backgroundColor: "transparent", fontFamily: "var(--font-mono, monospace)" }}
    />
  );
}

// A base-16 theme mapped onto the workbench's dark surface tokens, so the tree matches the rest of the
// result region (bg/fg/muted/accent) rather than shipping a foreign palette. Values read faithfully;
// only the chrome (keys, punctuation, type glyphs) is tinted.
const RULES_JSON_THEME = {
  base00: "transparent", // background
  base01: "hsl(var(--border))",
  base02: "hsl(var(--border))", // collapse guide / selection
  base03: "hsl(var(--muted))", // object-size label
  base04: "hsl(var(--muted))", // array index
  base05: "hsl(var(--fg))", // default text / punctuation
  base06: "hsl(var(--fg))",
  base07: "hsl(var(--fg))", // keys
  base08: "hsl(var(--muted))", // null / undefined / function
  base09: "hsl(var(--accent))", // strings
  base0A: "hsl(var(--accent))", // nan
  base0B: "hsl(var(--accent))", // floats
  base0C: "hsl(var(--accent))", // array-key glyph
  base0D: "hsl(var(--muted))", // expand/collapse arrows
  base0E: "hsl(var(--muted))", // booleans
  base0F: "hsl(var(--accent))", // ints
} as const;
