// `prefs.resolve` client verb (user-prefs scope) — fold the viewer's preference chain server-side and
// return the fully-resolved axes. Member-level + forced to the caller's own `sub` (a viewer can only
// ever resolve their OWN prefs — structural, not just cap-gated). The optional `override` is
// self-scoped (e.g. "preview in es"): it changes THIS response only, never writes the record.

import type { ResolvedPrefs } from "./prefs.types";
import { invoke } from "@/lib/ipc/invoke";

/** Resolve the current viewer's prefs. Mirrors the gateway `POST /prefs/resolve`. */
export function resolvePrefs(override?: Partial<ResolvedPrefs>): Promise<ResolvedPrefs> {
  return invoke<{ resolved: ResolvedPrefs }>("prefs_resolve", { override }).then((r) => r.resolved);
}
