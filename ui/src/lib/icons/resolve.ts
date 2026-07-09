// Resolve a stable string icon name to a lucide-react component, so an icon can be
// *stored* (in prefs, a dashboard cell, an ext descriptor) as opaque data and rendered
// later. lucide-react exports `icons`: a record keyed by PascalCase name (`AlarmCheck`)
// — that record IS our name→component map, no per-icon static import. We accept the
// friendlier kebab-case name developers see in the picker ("alarm-check") and normalise.
// One responsibility per file (FILE-LAYOUT): name→component only; the catalog is separate.

import { icons, type LucideIcon } from "lucide-react";

/** kebab-case ("arrow-down-a-z") → PascalCase ("ArrowDownAZ"-ish) key lucide uses. */
function toPascal(name: string): string {
  return name
    .split(/[-_\s]+/)
    .filter(Boolean)
    .map((p) => p.charAt(0).toUpperCase() + p.slice(1))
    .join("");
}

/**
 * Look up the lucide component for an icon name. Accepts kebab-case ("git-branch"),
 * the PascalCase key ("GitBranch"), or the `Lucide`-prefixed alias. Returns `null` for
 * an unknown name so callers can render a fallback instead of throwing.
 */
export function resolveIcon(name: string | null | undefined): LucideIcon | null {
  if (!name) return null;
  const record = icons as Record<string, LucideIcon>;
  if (record[name]) return record[name];
  const pascal = toPascal(name);
  return record[pascal] ?? record[`Lucide${pascal}`] ?? null;
}

/** Whether a name resolves to a real icon. */
export function isIconName(name: string): boolean {
  return resolveIcon(name) !== null;
}
