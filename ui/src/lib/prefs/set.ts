// `prefs.set` client verb (user-prefs scope) — merge a patch into the viewer's OWN prefs record
// (member-level, forced to the caller's `sub`; a viewer can never write another user's record). Each
// axis is nullable — omit an axis to leave it inheriting the chain. The settings surface calls this;
// tests use it to seed a real viewer preference through the real write path (no fake store).

import type { ResolvedPrefs } from "./prefs.types";
import { invoke } from "@/lib/ipc/invoke";

/** The settable subset — every axis optional (a patch). Mirrors `lb_prefs::Prefs` nullable fields. */
export type PrefsPatch = Partial<
  Pick<ResolvedPrefs, "language" | "timezone" | "date_style" | "time_style">
> & { unit_system?: string; number_format?: string };

/** Merge `patch` into the viewer's own prefs. Mirrors the gateway `PUT /prefs` (204). */
export function setPrefs(patch: PrefsPatch): Promise<void> {
  return invoke<void>("prefs_set", patch as Record<string, unknown>);
}
