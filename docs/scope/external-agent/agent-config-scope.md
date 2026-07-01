# Agent-config scope — the per-workspace active-agent setting

Status: scope (the ask). Promotes to `public/external-agent/` once shipped.

A workspace admin needs a place to **choose which agent runtime a workspace uses by default** and the
**model endpoint** it routes through — a persisted, per-workspace setting surfaced in a Settings UI —
rather than the choice living only in each node's boot config. The read side already exists
(`agent.runtimes` lists what a node *offers*); this adds the **write + persisted selection** so a
workspace can say "our default agent is `vtcode-default` on our Z.AI endpoint" and have every invoke
that omits an explicit `runtime` honor it.

## Goals

- **One agent-config record per workspace**, admin-settable, member-readable. It carries the chosen
  **default runtime id** (validated against the node's registry) and an optional **model endpoint**
  (`provider` / `model` / `api_key_env` / `base_url` — names, never secret values).
- **A read verb (`agent.config.get`)** the Settings UI (and, later, the invoke path) reads to know the
  active selection; **a write verb (`agent.config.set`, admin-gated)** that merges a patch.
- **A Settings surface** (workspace **Agent** tab) that lists the node's runtimes (`agent.runtimes`),
  shows the current selection, and lets an admin change it — cap-gated so a non-admin sees it read-only.

## Non-goals

- **Not the runtime registry itself.** What runtimes a node *can* run stays boot config + the
  compile-time `external-agent` feature (runtime-seam scope). This only records which of the offered
  ids is the workspace default and its endpoint — it never makes an unavailable runtime appear.
- **No secret values.** `api_key_env` is an env-var **name**; the actual key stays in the node env /
  `lb-secrets`, never in this record (mirrors `profiles.rs`' `ModelEndpoint`).
- **Not wiring the invoke default yet.** Honoring the stored default when `agent.invoke` omits
  `runtime` is a thin, separate follow-up (named below) — this slice ships the record + verbs + UI so
  the selection is real and visible; the invoke-path read is a one-line change once the record exists.
- **Not per-user.** The agent default is a **workspace** setting (like `workspace_prefs`), not a
  per-member one.

## Intent / approach

Mirror the **`prefs.set_default`** shape exactly — it is the proven "admin sets a per-workspace default
record" pattern. A `workspace_agent_config:[ws]` SCHEMAFULL record (composite id → idempotent offline
replay, LWW), a nullable-field patch (`MERGE`), two host verbs behind the standard `authorize_tool`
gate, two gateway routes (1:1 mirror), and a UI tab that reuses the shipped `agent.runtimes` picker.

**Rejected:** folding the agent default into `workspace_prefs`. Prefs are the localization axes (a
closed, generated enum set); the agent runtime is an orthogonal operational choice with its own
validation (against the node registry) and its own admin cap. Keeping it a distinct record keeps both
schemas honest (one responsibility per record).

## How it fits the core

- **Tenancy / isolation:** `workspace_agent_config:[ws]` lives in the workspace namespace, keyed
  `[ws]`. A read/write in ws-B can structurally never touch ws-A's record. Isolation tested with two
  workspaces holding different selections.
- **Capabilities:** `agent.config.get` is **member-level** (`mcp:agent.config.get:call` — a member must
  read it to render the Settings/Agent surface and, later, to know which runtime an invoke will use);
  `agent.config.set` is **admin-gated** (`mcp:agent.config.set:call`, beside `prefs.set_default` /
  `agent.policy.set`). Deny is opaque. Deny-test per verb: a non-admin `set` is denied; a member with
  neither cap reads nothing.
- **Placement:** `either` — a host record + verbs compiled into every node; no role branch. The chosen
  runtime id is validated against the node's own `RuntimeRegistry`, so a node that can't run an id
  rejects it at write time (a `BadInput`, not a silent accept).
- **MCP surface:** `agent.config.get` (read) + `agent.config.set` (admin write, MERGE patch). No
  `delete` (unset an axis by patching it null; the record is a single per-ws default, not a collection)
  — stated as a deliberate non-goal, not a gap. No live-feed (a config read on open + the existing
  "prefs changed"-style re-fetch on save is enough; the selection changes rarely). No batch (one record
  per ws). CRUD reduces to get + set here, and both are built.
- **Data (SurrealDB):** `workspace_agent_config` (SCHEMAFULL, composite id `[ws]`, nullable
  `default_runtime` + a flexible `model_endpoint` object). State, not motion.
- **Bus (Zenoh):** none — the config is state. A future "config changed" hint is ordinary motion the
  caller may publish; this slice does not need it.
- **Sync / authority:** cloud-authoritative shared workspace data with an edge read-cache; the
  deterministic composite id makes an offline edit idempotent on replay (LWW on a contested axis) —
  identical to `workspace_prefs`.
- **Secrets:** none stored (env-var **name** only).

## Example flow

1. A workspace admin opens **Settings → Agent**. The tab calls `agent.runtimes` → `{ default:
   "default", runtimes: ["default", "vtcode-default", …] }` and `agent.config.get` → `null` (unset).
2. The admin picks `vtcode-default` and fills the model endpoint (`provider: zaicoding`, `model:
   glm-4.6`, `api_key_env: ZAI_API_KEY`). The tab calls `agent.config.set { patch: { default_runtime:
   "vtcode-default", model_endpoint: {…} } }`. The host validates `vtcode-default` is in the node
   registry, then `UPSERT`s `workspace_agent_config:[ws]`.
3. A member reopens the tab; `agent.config.get` returns the stored selection (read-only for them, since
   they lack `agent.config.set`).
4. **(follow-up)** An `agent.invoke` that omits `runtime` reads this record and dispatches
   `vtcode-default` instead of the registry's compiled-in default.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`:

- **Capability deny** (per verb) — `agent.config.set` from a non-admin denied (opaque); a member
  lacking `agent.config.get` cannot read. Real gateway, real caps.
- **Workspace isolation — specified** — ws-A and ws-B each set a *different* `default_runtime`; a
  `get` in ws-B returns ws-B's value and never ws-A's, and a `set` in ws-A does not move ws-B.
- **Offline/sync** — a `set` replays idempotently (composite-id UPSERT, LWW) — asserted via a
  double-apply returning the same record.

Key cases: round-trip (`set` then `get` echoes the patch); unknown-runtime `set` rejected
(`BadInput`, validated against the registry); a null-axis patch clears that axis. Frontend: a
real-gateway `SettingsView` test — admin can pick + persist + re-read; a member sees it read-only.

## Risks & hard problems

- **Registry drift.** A stored `default_runtime` can name an id the node no longer offers (feature off,
  config changed). The write validates against the *current* registry; the read returns the stored id
  verbatim and the UI flags "not currently available" rather than erroring — so a config outlives a
  transient registry change without breaking the page. (The invoke-path follow-up must fall back to the
  registry default if the stored id is absent.)
- **Endpoint is names-only.** It is easy to accidentally accept a raw key; the schema stores only
  `api_key_env`. A test asserts no secret value round-trips.

## Open questions

- Should `agent.config.set` also accept `granted_tools`/`persona_skill` (the full `AgentProfile`
  surface from the umbrella)? **Deferred:** this slice ships runtime + endpoint (what a workspace picks
  today); the profile-authoring surface is its own scope when the external-agent feature ships in anger.

## Skill doc

Shipped: [`skills/external-agent/SKILL.md`](../../skills/external-agent/SKILL.md) — the operating
manual for setting up and driving the external agent end to end (the `external-agent` feature build,
`agent.runtimes`, the new `agent.config.get`/`set`, and `agent.invoke { runtime }` / the `/agent`
palette). It supersedes the earlier "N/A" call: once persistence made "set up the workspace agent" a
real, drivable admin task, it earned a runnable how-to grounded in a live run.

## Related

- [`external-agent-scope.md`](external-agent-scope.md) (umbrella — "What a profile is"),
  [`agent-runtimes-scope.md`](agent-runtimes-scope.md) (the read verb this pairs with),
  [`run-lifecycle-scope.md`](run-lifecycle-scope.md) (the invoke path the follow-up wires).
- `scope/prefs/user-prefs-scope.md` (the `prefs.set_default` pattern mirrored here) + its Settings-UI
  section (the sibling **Preferences** tab of the same Settings surface).
- README `§6.16` (shared AI agents), `§6.5` (MCP), `§7` (tenancy).
