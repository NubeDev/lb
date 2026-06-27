// The members api client — one call per export, mirroring `lb_host::list_members` /
// `add_team_member` and the gateway `GET|POST /teams/{team}/members` (collaboration scope, slice 3).

import { invoke } from "@/lib/ipc/invoke";

/** List the users in `team` (within the session workspace). Mirrors `members_list`. Returns the
 *  `user:…` ids. */
export function listMembers(team: string): Promise<string[]> {
  return invoke<string[]>("members_list", { team });
}

/** Add `user` to `team`. Mirrors `members_add`. */
export function addMember(team: string, user: string): Promise<void> {
  return invoke<void>("members_add", { team, user });
}

/** Remove `user` from `team` (idempotent). Mirrors `members_remove` / the gateway
 *  `DELETE /teams/{team}/members/{user}` (admin-crud scope). The destructive verb the collaboration
 *  `MembersView` lacked; the admin console routes it through `ConfirmDestructive`. */
export function removeMember(team: string, user: string): Promise<void> {
  return invoke<void>("members_remove", { team, user });
}
