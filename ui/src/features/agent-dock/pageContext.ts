// The page-context BUILDER (agent-dock scope) — derive the `{ surface, path, search }` object the
// dock captures PER MESSAGE at send time, from the router state the shell already knows. Pure: given a
// pathname + a raw search record, produce the tenant-stripped, typed context. No React here (the
// provider lives in `PageContextProvider.tsx`); this file owns the derivation so it is unit-testable.
//
// Scope decision 3: v1 ships ONLY the router-derived default — the provider is the seam a later
// feature overrides (active panel, focused cell). This builder is that default.

import { stripTenant, surfaceForPath } from "@/features/routing/surface";
import type { PageContext } from "@/lib/channel/payload.types";

/** Flatten a raw search record into flat `{ [k]: string }` — the wire shape. Arrays/objects are
 *  JSON-stringified, primitives coerced to strings; `undefined`/`null` entries are dropped. Keeps the
 *  context a shallow string map (the host fences it as opaque data; nothing here needs structure). */
function typeSearch(search: Record<string, unknown> | undefined): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(search ?? {})) {
    if (v === undefined || v === null) continue;
    out[k] = typeof v === "string" ? v : typeof v === "object" ? JSON.stringify(v) : String(v);
  }
  return out;
}

/** Build the page context from the current `pathname` and raw `search`. `surface` is `surfaceForPath`
 *  (opaque string — the host never branches on it, rule 10); `path` is tenant-stripped (no `/t/<ws>`);
 *  `search` is the flat typed map. Deterministic (no wall-clock) — captured fresh per send. */
export function buildPageContext(
  pathname: string,
  search: Record<string, unknown> | undefined,
): PageContext {
  return {
    surface: surfaceForPath(pathname),
    path: stripTenant(pathname),
    search: typeSearch(search),
  };
}
