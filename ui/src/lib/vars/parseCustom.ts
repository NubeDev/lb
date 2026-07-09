// Parse a `custom` variable's string list into `{text, value}` options (advanced-variables scope). A
// bare string is `{text:v, value:v}`; a `label : value` string splits display text from the interpolated
// value (Grafana's `text : value` custom syntax). Pure TS, no React.
//
// The split is on the FIRST unescaped ` : ` (space-colon-space, matching Grafana). `\:` is a literal
// colon in either side. A string with no ` : ` is a bare value (text = value).

import type { VariableOption } from "./types";

const SPLIT = /(?<!\\)\s:\s/;

/** Unescape `\:` → `:` in a parsed half. */
function unescape(s: string): string {
  return s.replace(/\\:/g, ":").trim();
}

/** Parse one custom string into `{text, value}`. */
export function parseCustomOption(raw: string): VariableOption {
  const m = SPLIT.exec(raw);
  if (!m) {
    const v = raw.trim();
    return { text: v, value: v };
  }
  const text = unescape(raw.slice(0, m.index));
  const value = unescape(raw.slice(m.index + m[0].length));
  return { text, value };
}

/** Parse a whole `custom` string list into options. */
export function parseCustomOptions(list: string[] | undefined): VariableOption[] {
  return (list ?? []).map(parseCustomOption);
}
