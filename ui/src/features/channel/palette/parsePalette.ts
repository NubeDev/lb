// PURE keystroke → structured palette state (channels-command-palette scope). No network, no React
// — this is the parser the palette unit-tests hammer. It classifies the input into one of three
// modes and ranks candidates; the host NEVER parses `/`/`@` text, the UI does (scope non-goal).
//
//   - command mode  — a leading `/` at line start: fuzzy-match a tool name; best is pre-selected.
//   - mention mode  — an `@` anywhere: fuzzy-match an entity for the active arg's picker.
//   - chat mode     — anything else: a plain message (no palette).
//
// Modes reclassify on the fly from the current text (one parser, no second component). Fuzzy
// ranking is a simple subsequence + contiguity score — good enough, deterministic, and testable.

import type { ToolDescriptor } from "@/lib/channel/palette.types";

/** The palette mode the current text resolves to. */
export type PaletteMode = "chat" | "command" | "mention";

/** A ranked candidate in the open menu (a tool in command mode, an entity in mention mode). */
export interface Candidate {
  /** The value inserted on accept (the tool name, or the entity id). */
  value: string;
  /** The label shown in the menu. */
  label: string;
  /** A secondary line (a tool's group/title, or an entity's reason). */
  hint?: string;
}

/** The structured parse of the current input. */
export interface PaletteParse {
  mode: PaletteMode;
  /** The fragment after the `/` or `@` sigil that is being matched (lowercased for matching). */
  query: string;
  /** Ranked candidates, best first (empty in chat mode). */
  candidates: Candidate[];
  /** The index pre-selected (0 when there are candidates, -1 otherwise). */
  selected: number;
}

/** Score `query` against `target` — a subsequence match with a contiguity/prefix bonus. Returns
 *  null when `query` is not a subsequence of `target` (no match). Lower is better. */
export function fuzzyScore(query: string, target: string): number | null {
  if (query === "") return 0;
  const q = query.toLowerCase();
  const t = target.toLowerCase();
  let ti = 0;
  let score = 0;
  let lastHit = -2;
  for (let qi = 0; qi < q.length; qi++) {
    const ch = q[qi];
    const at = t.indexOf(ch, ti);
    if (at < 0) return null;
    // Gap penalty: contiguous matches (at === lastHit+1) cost nothing; a jump costs its distance.
    score += at === lastHit + 1 ? 0 : at - ti + 1;
    if (at === 0) score -= 1; // prefix bonus
    lastHit = at;
    ti = at + 1;
  }
  return score;
}

/** Rank `items` by their fuzzy score against `query`, dropping non-matches; stable for ties (keeps
 *  the input order, so an empty query preserves the catalog's sort). */
function rank<T>(query: string, items: T[], key: (i: T) => string): T[] {
  return items
    .map((item, idx) => ({ item, idx, score: fuzzyScore(query, key(item)) }))
    .filter((s): s is { item: T; idx: number; score: number } => s.score !== null)
    .sort((a, b) => a.score - b.score || a.idx - b.idx)
    .map((s) => s.item);
}

/** Parse the composer `text` into palette state. `tools` is the cached catalog (command mode);
 *  `mentions` are the entity candidates for the active arg (mention mode), already loaded. Pure. */
export function parsePalette(
  text: string,
  tools: ToolDescriptor[],
  mentions: Candidate[] = [],
): PaletteParse {
  // Mention mode wins when an `@` is the last sigil before the caret-equivalent (end of text),
  // because an arg can be filled mid-command (`/query @wa`). Find the last `@` with no whitespace
  // after it.
  const at = lastSigil(text, "@");
  if (at >= 0) {
    const query = text.slice(at + 1);
    const candidates = rank(query, mentions, (m) => m.label || m.value);
    return { mode: "mention", query, candidates, selected: candidates.length ? 0 : -1 };
  }

  // Command mode: a `/` at line start (the whole text begins with `/`).
  if (text.startsWith("/")) {
    const query = text.slice(1);
    const candidates: Candidate[] = rank(query, tools, (d) => d.title || d.name).map((d) => ({
      value: d.name,
      label: d.title || d.name,
      hint: d.group || undefined,
    }));
    return { mode: "command", query, candidates, selected: candidates.length ? 0 : -1 };
  }

  return { mode: "chat", query: "", candidates: [], selected: -1 };
}

/** The index of the last `sigil` that has no whitespace between it and the end of `text` (so it is
 *  the token currently being typed). -1 when none — a completed `@chip ` no longer matches. */
function lastSigil(text: string, sigil: string): number {
  const at = text.lastIndexOf(sigil);
  if (at < 0) return -1;
  // No whitespace in the fragment after the sigil → it is the active token.
  return /\s/.test(text.slice(at + 1)) ? -1 : at;
}
