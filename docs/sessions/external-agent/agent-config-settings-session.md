# Settings surface — user preferences + workspace agent-config (session)

- Date: 2026-07-02
- Scope: ../../scope/external-agent/agent-config-scope.md (the new per-workspace agent record + verbs) ·
  ../../scope/prefs/user-prefs-scope.md ("How the UI handles this → The settings surface", the
  long-deferred client half)
- Builds on: ../prefs/lb-prefs-session.md (the shipped prefs verbs) ·
  ../channels/agent-runtimes-picker-session.md (the `agent.runtimes` read verb reused here)
- Stage: post-S10 (settings surface; new backend slice + the prefs client half)
- Status: done (a dedicated **Settings** nav surface with a Preferences tab over all eight prefs axes +
  admin workspace-defaults, and an Agent tab that persists the workspace's default runtime + model
  endpoint through two NEW admin/member host verbs)

## Goal — one place to "set up the agent" and see/edit preferences

The user asked for **workspace settings (set up the agent)** and **user settings/preferences** surfaces.
Two halves:

- **Preferences** was a *frontend-only* gap: the `lb-prefs` verbs (`prefs.get/set/resolve/set_default`)
  shipped long ago; the prefs scope explicitly names "the client settings/bootstrap-locale UI" as
  deferred. This session builds that editor — **all eight axes** (language, timezone, date/time style,
  first-day-of-week, number format, unit system, and the closed dimension→unit overrides), for the
  caller's own record AND (admin) the workspace default.
- **Agent** needed *new backend*: `agent.runtimes` only *lists* what a node offers; nothing persisted a
  workspace's *choice*. Added a `workspace_agent_config:[ws]` record + two verbs (`agent.config.get`
  member, `agent.config.set` admin), mirroring the `prefs.set_default` pattern exactly.

Exit gate: a new **Settings** nav entry (always visible — prefs are member-level) with two tabs;
Preferences saves/reads real records over the live gateway; Agent lets an admin pick a
registry-validated runtime + names-only endpoint and persists it, read-only for a non-admin; every new
verb has a deny-test; workspace isolation proven.

## What shipped

### Backend (agent-config)
- `rust/crates/host/src/agent/config/` (one responsibility per file, FILE-LAYOUT):
  - `model.rs` — `AgentConfig` + `ModelEndpointPatch` (all-nullable patch shapes; endpoint is
    **names only** — `api_key_env` is an env-var name, never a key).
  - `store.rs` — the SCHEMAFULL `workspace_agent_config:[ws]` table + raw get/set (composite-id
    `UPSERT ... MERGE`, idempotent replay — the `workspace_prefs` pattern).
  - `verbs.rs` — the gated verbs: `agent_config_get` (member, `mcp:agent.config.get:call`),
    `agent_config_set` (admin, `mcp:agent.config.set:call`) with **registry validation** (a
    `default_runtime` the node can't run → `BadInput`, not a silent accept).
  - `tool.rs` — the `agent.config.*` MCP bridge, composed into `call_agent_tool` (the `agent.` prefix
    branch) as a fall-through before `NotFound`.
- Gateway: `routes/agent_config.rs` (`GET|PUT /agent/config`, 1:1 mirror), registered in `server.rs`.
- Caps: `mcp:agent.config.get:call` + `mcp:agent.config.set:call` granted to the dev principal
  (`credentials.rs`), with a comment noting `set` is the admin half beside `prefs.set_default`.

### Frontend (Settings surface)
- New nav surface `settings` — added to `CoreSurface` + `SURFACES` (Settings gear icon), `CORE_PATHS`,
  `allowed.ts` (always allowed — every member edits their own prefs), and a `/settings` route +
  `SettingsPage` in `createAppRouter.tsx`.
- `features/settings/`: `SettingsView` (Tabs shell), `PreferencesTab` (all 8 axes; a "My preferences /
  Workspace defaults" scope switch shown only to an admin holding `prefs.set_default`; unit-override
  picker generated from `dimensions.generated.ts` so client/server can't disagree), `AgentTab` (runtime
  dropdown from `agent.runtimes` + names-only endpoint; disabled/read-only without
  `agent.config.set`; flags a stored-but-unavailable runtime as registry drift), `Field` (shared
  labeled-row primitive).
- API clients: `lib/agent/config.api.ts` (`getAgentConfig`/`setAgentConfig` over the `mcp_call`
  bridge, like `agent.runtimes`); `lib/prefs/get.ts` (`prefs.get`) + `setDefaultPrefs` in
  `lib/prefs/set.ts`; widened `PrefsPatch` to all eight axes; `http.ts` gained `prefs_get` /
  `prefs_set_default` cases (mirroring `GET /prefs` / `PUT /prefs/default`). New `CAP` entries:
  `prefsSet`, `prefsSetDefault`, `agentConfigGet`, `agentConfigSet`.

## Tests (real infra, seeded via the real write path — rule 9)

- **Backend** `cargo test -p lb-host --test agent_config_test` — **6 green**: round-trip (+names-only
  assertion), `set` deny without admin cap (opaque), `get` deny without read cap, **workspace
  isolation** (ws-A/ws-B different runtimes, no cross-read), unknown-runtime rejected (`BadInput`),
  double-apply idempotent (composite-id UPSERT). Boots a REAL `Node` (store + registry + gate real).
- **Frontend** `pnpm test:gateway SettingsView.gateway` — **3 green** against a spawned gateway node:
  Preferences round-trip (set language/date/unit-override → real `prefs.get` reads them → fresh mount
  hydrates), Agent round-trip (admin picks runtime + endpoint → real `agent.config.get`), and the
  capability gate (a member without `agent.config.set` sees the controls disabled AND the server denies
  a direct `setAgentConfig` — the UI gate is convenience, the server is the wall).

Green output pasted below.

```
# cargo test -p lb-host --test agent_config_test
running 6 tests
test workspaces_are_isolated ... ok
test get_without_the_read_cap_is_denied_opaquely ... ok
test setting_an_unknown_runtime_is_rejected ... ok
test double_apply_is_idempotent ... ok
test set_without_the_admin_cap_is_denied_opaquely ... ok
test set_then_get_round_trips_the_patch ... ok
test result: ok. 6 passed; 0 failed; ...

# pnpm test:gateway SettingsView.gateway
✓ src/features/settings/SettingsView.gateway.test.tsx (3 tests)
Test Files  1 passed (1)
     Tests  3 passed (3)
```

## Decisions & notes

- **Distinct record, not folded into `workspace_prefs`.** The agent runtime is an operational choice
  with its own admin cap + registry validation; prefs are the closed localization axes. Keeping them
  separate keeps both schemas honest (scope "Rejected").
- **UI reads via `mcp_call`, gateway routes still added.** The Settings UI reads/writes agent config
  over the generic `mcp_call` bridge (consistent with `agent.runtimes`), but `GET|PUT /agent/config`
  is also wired for the 1:1 house convention + non-browser clients.
- **`agent.config.get` is member-level** so the Agent tab renders for anyone and (follow-up) the invoke
  path can read the active runtime; only `set` is admin.
- **Settings is always in the nav** because every member can edit their own prefs; the admin-only
  controls (workspace-default scope switch, agent editor) are cap-gated per-control, server-enforced.
- **Pre-existing note (not touched):** `components/ui/tabs.tsx` calls `React.useId()` inside a
  `useMemo` (a hooks-rules warning surfaced by these tests). It predates this work and lives in a
  shared primitive; left for a focused ui-standards fix rather than widening this slice.
- **Environment note:** a concurrent Tailwind-v4 migration on this branch briefly broke PostCSS/
  autoprefixer resolution mid-session; unrelated to this work — tests pass once the migration churn
  settles (verified against an existing `WorkspacesAdmin.gateway` run).

## Follow-ups (named, not silent gaps)

- **Honor the stored default in `agent.invoke`** when `runtime` is omitted (read `agent.config.get`,
  fall back to the registry default if the stored id is unavailable). One-line read, its own slice
  (scope non-goal here).
- **Full `AgentProfile` authoring** (`granted_tools`/`persona_skill`) — deferred to when the
  external-agent feature ships in anger (scope open question).
