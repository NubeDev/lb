// The rules API client — one call per export, mirroring the gateway's `rules.*` routes and the host
// verbs 1:1 (rules-workbench scope, Phase 1). The UI never calls `invoke` directly; it goes through
// these named verbs (FILE-LAYOUT frontend rules). Each is capability-gated server-side; the workspace
// + principal come from the session token (the hard wall, §7), never an argument. A denied call throws
// a generic "not permitted"; author feedback (a cage/parse error, an AI-budget / AI-not-configured
// message) throws with the verbatim message from the host — the page renders it honestly.

import type { RunResult, SavedRule, RuleParam } from "./rules.types";
import { invoke } from "@/lib/ipc/invoke";

/** Run an ad-hoc (`body`) or saved (`ruleId`) rule with optional `params`. Mirrors `rules.run`. */
export function runRule(args: {
  body?: string;
  ruleId?: string;
  params?: Record<string, unknown>;
}): Promise<RunResult> {
  return invoke<RunResult>("rules_run", {
    body: args.body,
    rule_id: args.ruleId,
    params: args.params ?? {},
  });
}

/** Create or update a saved rule (idempotent UPSERT on `id`). Mirrors `rules.save`. Returns `{id}`. */
export function saveRule(args: {
  id: string;
  name?: string;
  body: string;
  params?: RuleParam[];
}): Promise<{ id: string }> {
  return invoke<{ id: string }>("rules_save", {
    id: args.id,
    name: args.name,
    body: args.body,
    params: args.params ?? [],
  });
}

/** Read one saved rule by id. Mirrors `rules.get`. */
export function getRule(id: string): Promise<SavedRule> {
  return invoke<SavedRule>("rules_get", { id });
}

/** The workspace's saved-rule roster. Mirrors `rules.list`. */
export function listRules(): Promise<SavedRule[]> {
  return invoke<{ rules: SavedRule[] }>("rules_list", {}).then((r) => r.rules);
}

/** Soft-delete a saved rule (idempotent tombstone). Mirrors `rules.delete`. */
export function deleteRule(id: string): Promise<void> {
  return invoke<void>("rules_delete", { id });
}
