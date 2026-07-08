// formatRhai — a lightweight re-indenter for Rhai rule bodies (rules-editor-ux scope). Rhai isn't
// JS, so a JS/AST formatter (prettier) isn't safe here — this instead re-flows on the language's
// actual structural tokens: statements split on top-level `;`, blocks open/close on `{`/`}`, and
// nesting depth drives indentation. It does not attempt to reformat expressions themselves, only
// statement/block layout — plus one targeted exception: a long SQL literal inside a `query(…)` call
// is beautified + wrapped (see `formatSqlLiterals`), because that's the other "one giant line" case.
// Good enough to turn a wall of text readable. One function per file (FILE-LAYOUT).

import { formatSqlLiterals } from "./formatSqlLiterals";

const INDENT = "  ";

/** Re-indent a Rhai source string. Strings and comments are tracked so `;`/`{`/`}` inside them are
 *  never treated as structural. */
export function formatRhai(source: string): string {
  const tokens = splitStatements(source);
  const lines: string[] = [];
  let depth = 0;

  for (const raw of tokens) {
    const token = raw.trim();
    if (!token) continue;

    const leadingCloses = token === "}" ? 1 : 0;
    depth = Math.max(0, depth - leadingCloses);

    const indent = INDENT.repeat(depth);
    lines.push(indent + formatSqlLiterals(token, indent));

    // A token is a block-opener when it ENDS in a bare `{` (e.g. `if x {`, or `{` alone) —
    // `splitStatements` already keeps `#{ ... }` map literals inline within a single token, so any
    // other `{`/`}` embedded mid-token belongs to a map literal, not a block, and must not perturb
    // `depth`.
    if (token.endsWith("{") && !token.endsWith("#{")) depth += 1;
  }

  return lines.join("\n").replace(/\n{3,}/g, "\n\n").trimEnd() + "\n";
}

/** Split source into structural chunks on `;`, `{`, `}` — outside of strings/comments — merging each
 *  closer/opener onto the statement it terminates so e.g. `if x { y; }` becomes three lines. A `{`
 *  immediately preceded by `#` (Rhai's object-map literal, `#{ a: 1 }`) is NOT a block delimiter — it
 *  and its matching `}` stay inline with the expression, tracked via `mapDepth` so nested maps and
 *  block braces inside a map (there are none in Rhai, but defensively) don't desync. */
function splitStatements(source: string): string[] {
  const chunks: string[] = [];
  let cur = "";
  let i = 0;
  let inString: '"' | "'" | null = null;
  let inRaw = false; // inside a Rhai backtick `…` raw string (no escapes, may span lines)
  let inLineComment = false;
  let mapDepth = 0;

  while (i < source.length) {
    const ch = source[i];
    const next = source[i + 1];

    if (inLineComment) {
      cur += ch;
      if (ch === "\n") inLineComment = false;
      i++;
      continue;
    }

    if (inRaw) {
      // Backtick raw strings have no escape sequences — they end only at the next backtick. Their
      // contents (newlines, `;`, `{`) are never structural, so a re-format leaves a wrapped SQL
      // literal intact (idempotency).
      cur += ch;
      if (ch === "`") inRaw = false;
      i++;
      continue;
    }

    if (inString) {
      cur += ch;
      if (ch === "\\" && next !== undefined) {
        cur += next;
        i += 2;
        continue;
      }
      if (ch === inString) inString = null;
      i++;
      continue;
    }

    if (ch === "`") {
      inRaw = true;
      cur += ch;
      i++;
      continue;
    }

    if (ch === '"' || ch === "'") {
      inString = ch;
      cur += ch;
      i++;
      continue;
    }

    if (ch === "/" && next === "/") {
      inLineComment = true;
      cur += ch;
      i++;
      continue;
    }

    if (ch === "#" && next === "{") {
      mapDepth++;
      cur += ch + next;
      i += 2;
      continue;
    }

    if (mapDepth > 0) {
      if (ch === "{") mapDepth++;
      if (ch === "}") mapDepth--;
      cur += ch;
      i++;
      continue;
    }

    if (ch === "{" || ch === "}" || ch === ";") {
      cur += ch;
      chunks.push(cur);
      cur = "";
      i++;
      continue;
    }

    if (ch === "\n") {
      // Collapse existing newlines/indentation — we re-flow layout from scratch.
      cur += cur.endsWith(" ") || cur.length === 0 ? "" : " ";
      i++;
      continue;
    }

    cur += ch;
    i++;
  }

  if (cur.trim()) chunks.push(cur);
  // Collapse runs of whitespace to re-flow layout — but NOT in a chunk holding a backtick raw
  // string, whose internal newlines/indentation are part of the (already-formatted) value.
  return chunks.map((c) => (c.includes("`") ? c.trim() : c.replace(/\s+/g, " ").trim()));
}
