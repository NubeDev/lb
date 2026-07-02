// `prefs.get` client verb (user-prefs scope) — read the viewer's OWN stored, nullable prefs record
// (member-level, forced to the caller's `sub`). Unlike `resolvePrefs` (which folds the whole chain to
// fully-decided axes), this returns only what the user has *explicitly set* — the right shape for the
// settings editor (an unset axis shows as "inherit", not the resolved fallback). `null` when unset.

import type { PrefsPatch } from "./set";
import { invoke } from "@/lib/ipc/invoke";

/** Read the viewer's own stored prefs (only their set axes; `null` if none). Mirrors `GET /prefs`. */
export function getPrefs(): Promise<PrefsPatch | null> {
  return invoke<{ prefs: PrefsPatch | null }>("prefs_get").then((r) => r.prefs ?? null);
}
