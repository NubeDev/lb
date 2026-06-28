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
  devkitTemplates: "mcp:devkit.templates:call",
  devkitScaffold: "mcp:devkit.scaffold:call",
  devkitInspect: "mcp:devkit.inspect:call",
  devkitBuild: "mcp:devkit.build:call",
  nativeInstall: "mcp:native.install:call",
  // data-console (Data page, the DB browser): admin-only — these relax the per-record membership
  // gate (gate 3), so the Data nav entry is shown only for a session holding `store.scan`.
  storeTables: "mcp:store.tables:call",
  storeScan: "mcp:store.scan:call",
  storeGraph: "mcp:store.graph:call",
  // system-map (System page, the topology + status console): admin-only — a system snapshot reads
  // across every subsystem of the workspace, so the nav entry shows only for a session holding
  // `system.overview`. The gateway re-checks every verb server-side regardless.
  systemOverview: "mcp:system.overview:call",
  systemTopology: "mcp:system.topology:call",
  // system-map subsystem detail: the per-subsystem detail verb a no-page card drills into. Gated as
  // the others — the detail view only opens when the session holds this cap.
  systemSubsystem: "mcp:system.subsystem:call",
  // data-console (Ingest page): member-level series verbs — the Ingest nav entry shows for any
  // session that may read/list series.
  seriesList: "mcp:series.list:call",
  ingestWrite: "mcp:ingest.write:call",
  // dashboard (Dashboards page): member-level — the nav entry shows for any session that may list
  // dashboards; gate 3 / ownership still decides which specific ones they see/edit.
  dashboardList: "mcp:dashboard.list:call",
  dashboardSave: "mcp:dashboard.save:call",
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
  CAP.devkitTemplates,
  CAP.devkitScaffold,
  CAP.devkitInspect,
  CAP.devkitBuild,
  CAP.nativeInstall,
  // data-console: the dev admin carries both the admin DB-browser caps and the member series caps.
  CAP.storeTables,
  CAP.storeScan,
  CAP.storeGraph,
  CAP.seriesList,
  CAP.ingestWrite,
  CAP.dashboardList,
  CAP.dashboardSave,
  "mcp:dashboard.get:call",
  "mcp:dashboard.delete:call",
  "mcp:dashboard.share:call",
  "mcp:series.read:call",
  "mcp:series.latest:call",
  "mcp:series.find:call",
  CAP.systemOverview,
  CAP.systemTopology,
  CAP.systemSubsystem,
];

/** Any one of these present → the admin section is shown (then per-control caps gate within it). */
export const ADMIN_SECTION_CAPS: string[] = [
  CAP.userManage,
  CAP.teamsManage,
  CAP.grantsAssign,
  CAP.workspaceDelete,
  CAP.extList,
  CAP.devkitTemplates,
];

/** Does `caps` include `cap`? The single cap-check the UI uses to gate a control's display. */
export function hasCap(caps: string[] | undefined, cap: string): boolean {
  return !!caps && caps.includes(cap);
}

/** Should the admin section be shown at all? True if the session carries ANY admin cap. */
export function isAdmin(caps: string[] | undefined): boolean {
  return !!caps && ADMIN_SECTION_CAPS.some((c) => caps.includes(c));
}
