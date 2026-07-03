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
  // access-console scope — the three verbs that close the access-graph gaps. Admin-only; the
  // gateway re-checks each server-side. `authzResolve` reveals the effective-caps detail + overview;
  // `authzRevokeTokens` reveals the live-token revoke lever; `rolesManage` reveals roles.delete.
  authzResolve: "mcp:authz.resolve:call",
  authzRevokeTokens: "mcp:authz.revoke-tokens:call",
  rolesManage: "mcp:roles.manage:call",
  // global-identity scope — the identity directory + per-workspace membership roster. The People tab
  // reads `membership.list`; the switcher reads `identity.workspaces`. The gateway re-checks each.
  identityManage: "mcp:identity.manage:call",
  membersManage: "mcp:members.manage:call",
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
  dashboardGet: "mcp:dashboard.get:call",
  dashboardSave: "mcp:dashboard.save:call",
  // library panels (library-panels scope): the reusable + standalone panel asset. Member-level like
  // dashboards — `panelGet` gates the standalone `/panel/{id}` page + the editor's link/unlink reads;
  // `panelSave` gates Save-as-library. Sharing a panel shares its DEFINITION only — its `sources[]`
  // re-check under the viewer's caps at render (the gateway re-checks every verb regardless).
  panelGet: "mcp:panel.get:call",
  panelList: "mcp:panel.list:call",
  panelSave: "mcp:panel.save:call",
  panelDelete: "mcp:panel.delete:call",
  panelShare: "mcp:panel.share:call",
  panelUsage: "mcp:panel.usage:call",
  // nav builder (nav scope): the user-/team-authored navigation menu. The reads (`list`/`get`/
  // `resolve`) are member-level — every member resolves their own menu; `navSave`/`navShare` are the
  // admin-ish authoring caps that gate the builder tab (revocable like any grant). The nav grants
  // NOTHING — `nav.resolve` is a pure lens; the gateway re-checks every page verb server-side.
  navList: "mcp:nav.list:call",
  navGet: "mcp:nav.get:call",
  navSave: "mcp:nav.save:call",
  navDelete: "mcp:nav.delete:call",
  navShare: "mcp:nav.share:call",
  navResolve: "mcp:nav.resolve:call",
  // ui-layout (data-studio scope v2): the member-owned per-surface workbench layout. Member-level —
  // `layoutSet` gates the debounced save of the caller's OWN Data Studio arrangement (the verb keys
  // the record to the token `sub`); the gateway re-checks both verbs server-side regardless.
  layoutGet: "mcp:layout.get:call",
  layoutSet: "mcp:layout.set:call",
  // rules workbench (rules-workbench scope): member-level nav gates. The Playground shows for any
  // session that may run a rule; the datasources admin for datasource.list. Gate 3 / ownership + the
  // gateway's per-verb re-check are the real boundary. (The DAG canvas is Flows — see `flowsList`.)
  rulesRun: "mcp:rules.run:call",
  datasourceList: "mcp:datasource.list:call",
  // flows (flows-canvas scope, Wave 3): member-level nav gate. The canvas shows for any session that
  // may list flows; the gateway re-checks `mcp:flows.<verb>:call` per verb server-side regardless.
  flowsList: "mcp:flows.list:call",
  // reminders (reminders scope): the nav gate. The page shows for a session that may list reminders;
  // the gateway re-checks `mcp:reminder.<verb>:call` per verb server-side regardless.
  reminderList: "mcp:reminder.list:call",
  // api-keys (api-keys scope): the machine-credential management verb gate. The tab shows for a
  // session holding `apikey.manage`; the gateway re-checks every verb server-side regardless.
  apikeyManage: "mcp:apikey.manage:call",
  // telemetry console (telemetry-console scope): the nav gate. The Telemetry page shows for a session
  // that may read the capped telemetry ring; the gateway re-checks `mcp:telemetry.read:call` on every
  // query/tail server-side and hard-filters to the caller's workspace. The audit lane needs its own
  // grant too (the console requires BOTH to show both lanes — `auditQuery`).
  telemetryRead: "mcp:telemetry.read:call",
  auditQuery: "mcp:audit.query:call",
  // settings (user-prefs + agent-config scopes): the Settings page's Preferences tab is member-level
  // (`prefs.set` writes the caller's OWN record); the workspace-default control gates on the
  // admin-only `prefs.set_default`. The Agent tab reads with `agent.config.get` (member) and the
  // admin-only `agent.config.set` gates the runtime/endpoint editor. The gateway re-checks each.
  prefsSet: "mcp:prefs.set:call",
  prefsSetDefault: "mcp:prefs.set_default:call",
  agentConfigGet: "mcp:agent.config.get:call",
  agentConfigSet: "mcp:agent.config.set:call",
  // agent-catalog scope: the definition catalog. `list`/`get` are member-level (the picker reads
  // them); create/update/delete are admin (custom definitions only — built-ins are read-only).
  agentDefList: "mcp:agent.def.list:call",
  agentDefGet: "mcp:agent.def.get:call",
  agentDefCreate: "mcp:agent.def.create:call",
  agentDefUpdate: "mcp:agent.def.update:call",
  agentDefDelete: "mcp:agent.def.delete:call",
  // agent-catalog test-and-secrets scope: the context-proving diagnostic. Its OWN admin-tier cap
  // (distinct from the read-ish `list`) because the test spends a model turn.
  agentDefTest: "mcp:agent.def.test:call",
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
  CAP.membersManage,
];

/** Does `caps` include `cap`? The single cap-check the UI uses to gate a control's display. */
export function hasCap(caps: string[] | undefined, cap: string): boolean {
  return !!caps && caps.includes(cap);
}

/** Should the admin section be shown at all? True if the session carries ANY admin cap. */
export function isAdmin(caps: string[] | undefined): boolean {
  return !!caps && ADMIN_SECTION_CAPS.some((c) => caps.includes(c));
}
