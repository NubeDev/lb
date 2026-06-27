// The teams admin api client — one call per export, mirroring the host `teams` service verbs and the
// gateway `/admin/teams` routes 1:1 (admin-crud scope). Membership edges live in the members api;
// these are the team *records* (create/list/rename/delete). The workspace is the session's.

import { invoke } from "@/lib/ipc/invoke";

export interface TeamView {
  team: string;
  name: string;
}

/** List the workspace's team records. Mirrors `teams.list`. */
export function listTeams(): Promise<TeamView[]> {
  return invoke<TeamView[]>("teams_list", {});
}

/** Create team `team` with display `name`. Mirrors `teams.create`. */
export function createTeam(team: string, name: string): Promise<void> {
  return invoke<void>("teams_create", { team, name });
}

/** Rename team `team` to `name`. Mirrors `teams.rename`. */
export function renameTeam(team: string, name: string): Promise<void> {
  return invoke<void>("teams_rename", { team, name });
}

/** Delete `team` — cascade: drop member edges + revoke the team's grants + tombstone the record.
 *  Returns the count of members removed (the consequence the confirm shows). Mirrors `teams.delete`. */
export function deleteTeam(team: string): Promise<number> {
  return invoke<number>("teams_delete", { team });
}
