// The users admin api client — one call per export, mirroring the host `users` service verbs and the
// gateway `/admin/users` routes 1:1 (admin-crud scope). The workspace is the session's (the gateway
// derives it from the token); never passed in the request. `cred_ref` is never returned (UserView).

import { invoke } from "@/lib/ipc/invoke";

export interface UserView {
  user: string;
  active: boolean;
  role: string;
}

/** List the workspace's users (no credential ever leaves the host). Mirrors `user.list`. */
export function listUsers(): Promise<UserView[]> {
  return invoke<UserView[]>("user_list", {});
}

/** Create `user` (seeds a dev credential). Mirrors `user.create`. */
export function createUser(user: string, role?: string): Promise<void> {
  return invoke<void>("user_create", { user, role });
}

/** Disable `user` — refused at next login until re-enabled. Mirrors `user.disable`. */
export function disableUser(user: string): Promise<void> {
  return invoke<void>("user_disable", { user });
}

/** Re-enable a disabled `user`. Mirrors `user.enable`. */
export function enableUser(user: string): Promise<void> {
  return invoke<void>("user_enable", { user });
}

/** Delete `user` (tombstone + revoke all their grants). Returns the count of revoked grants. */
export function deleteUser(user: string): Promise<number> {
  return invoke<number>("user_delete", { user });
}
