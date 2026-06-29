// The admin capability strings — mirrors the gateway's dev `member_caps()` admin half
// (`role/gateway/src/session/credentials.rs`). These are the caps the UI reads to decide which
// admin controls to *show*. The gateway re-checks each verb server-side; this list is a convenience
// for cap-gating display, NEVER the security boundary (admin-console scope).
//
// `ADMIN_SECTION_CAPS` is the set whose presence (any one) reveals the admin section at all (the
// scope's lean: show the section if *any* admin cap is present, then gate individual controls per cap).
//
// The real caps a session holds come from the gateway's `POST /login` reply (server-side
// `credentials.rs`), never a client list — there is deliberately NO client copy of the full grant
// here (a parallel re-implementation of node behavior is the banned-fake smell, CLAUDE §9). This file
// only names the cap STRINGS (`CAP`) the UI compares the server-issued caps against to gate display.

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
  // tool-catalog (MCP & ACP service pages, drilled from the System page): the reachable MCP tool
  // catalog + the ACP adapter's static facts. Admin-only by the same convention as the rest of the map.
  systemTools: "mcp:system.tools:call",
  systemAcp: "mcp:system.acp:call",
  // data-console (Ingest page): member-level series verbs — the Ingest nav entry shows for any
  // session that may read/list series.
  seriesList: "mcp:series.list:call",
  ingestWrite: "mcp:ingest.write:call",
  // dashboard (Dashboards page): member-level — the nav entry shows for any session that may list
  // dashboards; gate 3 / ownership still decides which specific ones they see/edit.
  dashboardList: "mcp:dashboard.list:call",
  dashboardSave: "mcp:dashboard.save:call",
  // rules workbench (rules-workbench scope): member-level nav gates. The Playground shows for any
  // session that may run a rule; the chain canvas for chains.get; the datasources admin for
  // datasource.list. Gate 3 / ownership + the gateway's per-verb re-check are the real boundary.
  rulesRun: "mcp:rules.run:call",
  chainsGet: "mcp:chains.get:call",
  datasourceList: "mcp:datasource.list:call",
  // reminders (reminders scope): the nav gate. The page shows for a session that may list reminders;
  // the gateway re-checks `mcp:reminder.<verb>:call` per verb server-side regardless.
  reminderList: "mcp:reminder.list:call",
  // api-keys (api-keys scope): the machine-credential management verb gate. The tab shows for a
  // session holding `apikey.manage`; the gateway re-checks every verb server-side regardless.
  apikeyManage: "mcp:apikey.manage:call",
} as const;

/** Any one of these present → the admin section is shown (then per-control caps gate within it). */
export const ADMIN_SECTION_CAPS: string[] = [
  CAP.userManage,
  CAP.teamsManage,
  CAP.grantsAssign,
  CAP.workspaceDelete,
  CAP.extList,
  CAP.devkitTemplates,
  CAP.apikeyManage,
];

/** Does `caps` include `cap`? The single cap-check the UI uses to gate a control's display. */
export function hasCap(caps: string[] | undefined, cap: string): boolean {
  return !!caps && caps.includes(cap);
}

/** Should the admin section be shown at all? True if the session carries ANY admin cap. */
export function isAdmin(caps: string[] | undefined): boolean {
  return !!caps && ADMIN_SECTION_CAPS.some((c) => caps.includes(c));
}
