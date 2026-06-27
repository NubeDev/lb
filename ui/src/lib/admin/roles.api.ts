// The roles admin api client — list + define custom role bundles (admin-console redesign). Mirrors
// the host `roles.*` verbs and the gateway `/admin/roles` routes 1:1. A role is a named bundle of
// capability strings; assigning a role to a subject is a grant of `role:<name>` (see grants.api).
// `define` is no-widening server-side: you may only bundle caps you yourself hold.

import { invoke } from "@/lib/ipc/invoke";

/** A role: a named bundle of capabilities (mirrors `lb_authz::Role`; the host's `kind` is ignored). */
export interface RoleView {
  name: string;
  caps: string[];
}

/** List the workspace's roles WITH the caps each bundles. Mirrors `roles.list`. */
export function listRoles(): Promise<RoleView[]> {
  return invoke<RoleView[]>("roles_list", {});
}

/** Define (or replace) role `name` bundling `caps`. Mirrors `roles.define` (no-widening server-side). */
export function defineRole(name: string, caps: string[]): Promise<void> {
  return invoke<void>("roles_define", { name, caps });
}
