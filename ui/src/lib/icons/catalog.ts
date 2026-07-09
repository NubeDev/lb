// The searchable icon catalog: every lucide icon as a stable kebab-case name plus
// search tokens. Derived from lucide's own `icons` record keys (converted PascalCase →
// kebab-case) so it stays complete as lucide grows — no hand-maintained list to rot.
// Aliases keyed off lucide's multi-name exports would need the raw export table; instead
// we split the name into word tokens so "arrow-down-a-z" matches "arrow", "down", "z".
// One responsibility per file (FILE-LAYOUT): catalog + search; rendering lives elsewhere.

import { icons, type LucideIcon } from "lucide-react";

export interface IconEntry {
  /** Stable kebab-case name — what you store and pass to `resolveIcon`. */
  name: string;
  /** The resolved component (present so the picker renders without a second lookup). */
  Icon: LucideIcon;
  /** Lower-case word tokens the search matches against. */
  tokens: string[];
}

/** PascalCase lucide key → kebab-case ("ArrowDownAZ" → "arrow-down-a-z"). */
function toKebab(key: string): string {
  return key
    .replace(/([a-z0-9])([A-Z])/g, "$1-$2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1-$2")
    .toLowerCase();
}

/** Build once at module load. ~1500 entries; a flat array is plenty for in-memory filter. */
function build(): IconEntry[] {
  const record = icons as Record<string, LucideIcon>;
  const seen = new Set<string>();
  const out: IconEntry[] = [];
  for (const key of Object.keys(record)) {
    // Skip lucide's `Lucide`-prefixed and `Icon`-suffixed alias keys — one entry per glyph.
    if (key.startsWith("Lucide") || key.endsWith("Icon")) continue;
    const name = toKebab(key);
    if (seen.has(name)) continue;
    seen.add(name);
    out.push({ name, Icon: record[key], tokens: name.split("-").filter(Boolean) });
  }
  out.sort((a, b) => a.name.localeCompare(b.name));
  return out;
}

export const ICON_CATALOG: IconEntry[] = build();

/**
 * Case-insensitive prefix/substring search over icon names. Ranks whole-name matches and
 * token-prefix matches above loose substring hits so "chart" surfaces `bar-chart` early.
 */
export function searchIcons(query: string, limit = 200): IconEntry[] {
  const q = query.trim().toLowerCase();
  if (!q) return ICON_CATALOG.slice(0, limit);
  const scored: Array<{ e: IconEntry; score: number }> = [];
  for (const e of ICON_CATALOG) {
    let score = 0;
    if (e.name === q) score = 100;
    else if (e.name.startsWith(q)) score = 80;
    else if (e.tokens.some((t) => t.startsWith(q))) score = 60;
    else if (e.name.includes(q)) score = 40;
    if (score > 0) scored.push({ e, score });
  }
  scored.sort((a, b) => b.score - a.score || a.e.name.localeCompare(b.e.name));
  return scored.slice(0, limit).map((s) => s.e);
}
