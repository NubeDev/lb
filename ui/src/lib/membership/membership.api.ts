// The membership api client — one call per export, mirroring `lb_host::membership_*` and the gateway
// `/admin/members*` routes 1:1 (global-identity scope). The per-workspace roster; gated
// `mcp:members.manage:call`. The Access console "People" tab reads `listMembers` (decision #9).

import { invoke } from "@/lib/ipc/invoke";

export interface MembershipView {
  sub: string;
  joined_ts: number;
  display_name?: string;
}

/** The effective roster of the session's workspace (membership ∪ legacy users). Mirrors `membership.list`. */
export function listMembers(): Promise<MembershipView[]> {
  return invoke<MembershipView[]>("membership_list", {});
}

/** Add a global identity to this workspace (grants the `member` role). Mirrors `membership.add`. */
export function addMember(sub: string): Promise<void> {
  return invoke<void>("membership_add", { sub });
}

/** Remove a member (tombstone + revoke_subject + revoke_tokens — a clean exit). Mirrors `membership.remove`. */
export function removeMember(sub: string): Promise<number> {
  return invoke<number>("membership_remove", { sub });
}
