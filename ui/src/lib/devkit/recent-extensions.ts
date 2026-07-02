// A short MRU list of extensions the user has opened/built, persisted in localStorage so reopening
// the Studio offers one-click rebuilds instead of re-walking the folder picker every time. Keyed by
// absolute path (the only thing a build actually needs); id/tier are cached for a readable label.
// Pure + storage-only — the wizard records entries, the Create step renders them.

const KEY = "lb.studio.recent-extensions";
const MAX = 5;

export interface RecentExtension {
  path: string;
  id: string;
  tier: "wasm" | "native";
  /** Epoch ms of the last open/build — for MRU ordering and a "2h ago" hint. */
  at: number;
}

export function loadRecent(): RecentExtension[] {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(isRecent).slice(0, MAX);
  } catch {
    // A corrupt/again-unavailable store must never break the page — just start empty.
    return [];
  }
}

/** Record an open/build. Moves an existing path to the front (MRU), caps at MAX, returns the new
 *  list so the caller can update state without a re-read. */
export function recordRecent(
  entry: Omit<RecentExtension, "at">,
  now: number,
): RecentExtension[] {
  const next: RecentExtension = { ...entry, at: now };
  const deduped = loadRecent().filter((r) => r.path !== entry.path);
  const list = [next, ...deduped].slice(0, MAX);
  save(list);
  return list;
}

export function removeRecent(path: string): RecentExtension[] {
  const list = loadRecent().filter((r) => r.path !== path);
  save(list);
  return list;
}

function save(list: RecentExtension[]): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(list));
  } catch {
    // Quota/private-mode failures are non-fatal — the feature is a convenience, not a source of truth.
  }
}

function isRecent(v: unknown): v is RecentExtension {
  if (typeof v !== "object" || v === null) return false;
  const r = v as Record<string, unknown>;
  return (
    typeof r.path === "string" &&
    typeof r.id === "string" &&
    (r.tier === "wasm" || r.tier === "native") &&
    typeof r.at === "number"
  );
}
