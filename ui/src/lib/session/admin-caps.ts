// The admin capability strings — mirrors the gateway's dev `member_caps()` admin half
// (`role/gateway/src/session/credentials.rs`). These are the caps the UI reads to decide which
// admin controls to *show*. The gateway re-checks each verb server-side; this list is a convenience
// for cap-gating display, NEVER the security boundary (admin-console scope).
//
// `ADMIN_CAPS` is the full dev-admin grant the fake hands back at login. `ADMIN_SECTION_CAPS` is the
// set whose presence (any one) reveals the admin section at all (the scope's lean: show the section
// if *any* admin cap is present, then gate individual controls per cap).

export const CAP = {
  workspaceDelete: "mcp:workspace.delete:call",
  workspacePurge: "mcp:workspace.purge:call",
  userManage: "mcp:user.manage:call",
  userDisable: "mcp:user.disable:call",
  teamsManage: "mcp:teams.manage:call",
  teamsList: "mcp:teams.list:call",
  grantsAssign: "mcp:grants.assign:call",
  grantsList: "mcp:grants.list:call",
  rolesDefine: "mcp:roles.define:call",
  rolesList: "mcp:roles.list:call",
  extList: "mcp:ext.list:call",
  extDisable: "mcp:ext.disable:call",
  extUninstall: "mcp:ext.uninstall:call",
} as const;

/** The full dev-admin cap grant (the gateway's `member_caps()` admin half + the ext caps). */
export const ADMIN_CAPS: string[] = [
  "bus:chan/*:pub",
  "bus:chan/*:sub",
  "mcp:members.list:call",
  "mcp:members.add:call",
  "mcp:inbox.list:call",
  "mcp:inbox.resolve:call",
  "mcp:outbox.status:call",
  "mcp:workspace.list:call",
  "mcp:workspace.create:call",
  CAP.workspaceDelete,
  CAP.workspacePurge,
  CAP.userManage,
  CAP.userDisable,
  CAP.teamsManage,
  CAP.teamsList,
  CAP.grantsAssign,
  CAP.grantsList,
  CAP.rolesDefine,
  CAP.rolesList,
  CAP.extList,
  CAP.extDisable,
  CAP.extUninstall,
];

/** Any one of these present → the admin section is shown (then per-control caps gate within it). */
export const ADMIN_SECTION_CAPS: string[] = [
  CAP.userManage,
  CAP.teamsManage,
  CAP.grantsAssign,
  CAP.workspaceDelete,
  CAP.extList,
];

/** Does `caps` include `cap`? The single cap-check the UI uses to gate a control's display. */
export function hasCap(caps: string[] | undefined, cap: string): boolean {
  return !!caps && caps.includes(cap);
}

/** Should the admin section be shown at all? True if the session carries ANY admin cap. */
export function isAdmin(caps: string[] | undefined): boolean {
  return !!caps && ADMIN_SECTION_CAPS.some((c) => caps.includes(c));
}
