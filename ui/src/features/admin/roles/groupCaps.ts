// Bucket a flat list of capability strings into collapsible groups for the Roles editor
// (access-console UX). A cap is `mcp:<id>.<verb>:call` with a dot-segmented `<id>` (auth-caps
// grammar); the FIRST id segment is the group (the extension), and the row shows only the
// short remainder — so the 209-cap checklist reads as a navigable tree instead of a wall of
// `mcp:…:call`. Non-`mcp:` caps (a `store:`/`bus:` string can enter via a role's own list) fall
// into an "other" group and keep their full string. Pure + deterministic (one responsibility,
// FILE-LAYOUT) so it unit-tests without a DOM.

/** One selectable capability: the FULL cap (for the checkbox value + aria-label) and a short
 *  display label (the `mcp:`/`:call` stripped remainder, the group prefix dropped). */
export interface CapEntry {
  cap: string;
  label: string;
}

/** A named bucket of caps — the group is the extension (first id segment), `*` for wildcards,
 *  or `other` for non-`mcp:` surfaces. */
export interface CapGroup {
  group: string;
  caps: CapEntry[];
}

const WILDCARD = "*";
const OTHER = "other";

/** Parse one cap into `{ group, label }`. Never throws and never yields an empty label. */
function parse(cap: string): { group: string; label: string } {
  // Non-mcp surfaces (store:/bus:/secret:) — keep the whole string, bucket under `other`.
  if (!cap.startsWith("mcp:")) {
    return { group: OTHER, label: cap };
  }
  // Strip the `mcp:` prefix and a trailing `:call` (only if present — degrade gracefully).
  let remainder = cap.slice("mcp:".length);
  if (remainder.endsWith(":call")) {
    remainder = remainder.slice(0, -":call".length);
  }
  const dot = remainder.indexOf(".");
  if (dot === -1) {
    // Single-segment id (e.g. `mcp:roles:call`) — the id IS the group; label stays the id so
    // the row is never blank.
    const group = remainder === WILDCARD ? WILDCARD : remainder;
    return { group, label: remainder };
  }
  const group = remainder.slice(0, dot);
  const label = remainder.slice(dot + 1);
  return { group, label };
}

/** Sort key for groups: named groups alphabetical first, then `*`, then `other` last. */
function groupRank(group: string): number {
  if (group === WILDCARD) return 1;
  if (group === OTHER) return 2;
  return 0;
}

/** Bucket caps by extension into ordered, deterministic groups. Caps within a group are sorted
 *  by their full string; groups are alphabetical with `*` and `other` pinned last. */
export function groupCaps(caps: string[]): CapGroup[] {
  const byGroup = new Map<string, CapEntry[]>();
  for (const cap of caps) {
    const { group, label } = parse(cap);
    const bucket = byGroup.get(group) ?? [];
    bucket.push({ cap, label });
    byGroup.set(group, bucket);
  }
  const groups: CapGroup[] = [...byGroup.entries()].map(([group, entries]) => ({
    group,
    caps: entries.sort((a, b) => a.cap.localeCompare(b.cap)),
  }));
  groups.sort((a, b) => {
    const r = groupRank(a.group) - groupRank(b.group);
    return r !== 0 ? r : a.group.localeCompare(b.group);
  });
  return groups;
}
