// `prefs.set` client verb (user-prefs scope) — merge a patch into the viewer's OWN prefs record
// (member-level, forced to the caller's `sub`; a viewer can never write another user's record). Each
// axis is nullable — omit an axis to leave it inheriting the chain. The settings surface calls this;
// tests use it to seed a real viewer preference through the real write path (no fake store).

import type { ResolvedPrefs } from "./prefs.types";
import { invoke } from "@/lib/ipc/invoke";

/** The settable patch — the nullable axes the viewer may write on their own (or the workspace-default)
 *  prefs record. Each axis optional: omit one to leave it inheriting the chain. Mirrors `lb_prefs::Prefs`
 *  nullable fields 1:1 (the closed dimension→unit `unit_overrides` map, the opaque `ui_theme` blob,
 *  the `insight_notifications` kill switch, and the `agent_persona` default focus from persona-session #5).
 *  `agent_persona` rides Prefs (not ResolvedPrefs — a nullable axis, not a folded i18n value); an empty
 *  string `""` clears the axis (the MERGE-can't-write-null workaround — the consumer's
 *  `filter(|s| !s.is_empty())` treats it as unset). */
export type PrefsPatch = Partial<
  Pick<
    ResolvedPrefs,
    | "language"
    | "timezone"
    | "date_style"
    | "time_style"
    | "first_day_of_week"
    | "number_format"
    | "unit_system"
    | "unit_overrides"
    | "ui_theme"
  >
> & {
  /** The viewer's default agent-persona id (persona-session #5). Opaque data; a dangling id warns +
   *  runs un-narrowed at the consumer, never an errored run. `""` clears the axis. */
  agent_persona?: string;
};

/** Merge `patch` into the viewer's own prefs. Mirrors the gateway `PUT /prefs` (204). */
export function setPrefs(patch: PrefsPatch): Promise<void> {
  return invoke<void>("prefs_set", patch as Record<string, unknown>);
}

/** Merge `patch` into the WORKSPACE-default prefs (admin-gated). Mirrors `PUT /prefs/default` (204). */
export function setDefaultPrefs(patch: PrefsPatch): Promise<void> {
  return invoke<void>("prefs_set_default", patch as Record<string, unknown>);
}
