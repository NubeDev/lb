# Agent-catalog scope — a manageable catalog of named agent definitions (seeded + custom)

Status: scope (the ask). Promotes to `public/agent/agent.md` once shipped.

Today a workspace can store exactly **one** agent selection (`agent.config` = one `default_runtime` +
one inline `model_endpoint`), typed into a raw form. There is no **library** of named, pickable agent
presets and nothing predefined out of the box. We want a **catalog of agent definitions** — each a
named bundle of `(runtime, model endpoint)` — that a workspace admin can **manage** (create / update /
delete custom ones) and **pick** from, with a set of **seeded predefined** definitions shipped by
default: the in-house **default** runtime and **Open Interpreter** (internal + external), each over
Z.AI **GLM-4.6 / GLM-5.1 / GLM-5.2**. The seed is a **TOML manifest** boot-seeded into a reserved
system namespace (the shipped core-skills pattern), so a fresh node has a working catalog with zero
setup and a node upgrade re-seeds cleanly.

## Goals

- **An `agent_definition` catalog.** A named preset = `{ id, label, description?, runtime,
  model_endpoint{provider,model,api_key_env,base_url} }` (endpoint is **names only**, never a secret).
  Two tiers, one shape: **built-in** (seeded, read-only to users) and **custom** (workspace-authored,
  full CRUD) — exactly the core-skills `core.*` / user split.
- **Seeded predefined definitions, by default, from a TOML manifest.** Boot-seeds the built-ins into a
  reserved namespace (`_lb_agents`, mirroring `_lb_skills`) from an embedded `agents.toml`. Ships:
  `in-house` (runtime `default`) and `open-interpreter` (runtime `open-interpreter-default`) × Z.AI
  `glm-4.6` / `glm-5.1` / `glm-5.2` — six built-ins over the `zaicoding` coding endpoint
  (`ZAI_API_KEY`). Idempotent re-seed; a node upgrade seeds new versions/entries.
- **The full management surface (MCP + UI).** Read verbs `agent.def.list` / `agent.def.get` (member);
  write verbs `agent.def.create` / `agent.def.update` / `agent.def.delete` (admin) over **custom**
  definitions only — a built-in `builtin.*` id is read-only regardless of caps (the core-skills
  `Reserved → BadInput` rule). The Settings **Agent** tab becomes a catalog manager + active-selection
  picker.
- **Pick a definition as the workspace default.** Selecting a catalog entry writes the existing
  `agent.config` (`default_runtime` + `model_endpoint` resolved from the definition), so the shipped
  `resolve_effective_runtime` invoke path honors it with **no new resolution seam**. The catalog is the
  *library*; `agent.config` stays the one *active* selection.

## Non-goals

- **Not a new runtime registry.** What runtimes a node can *run* stays the compile-time
  `external-agent` feature + the boot registry (runtime-seam / default-agent-wiring scopes). A built-in
  whose `runtime` the node doesn't offer is **filtered out of the list** (registry drift, exactly as
  the Agent tab already flags a stored-but-unavailable runtime) — the catalog never makes an
  unrunnable runtime appear.
- **No secret values.** `api_key_env` is an env-var **NAME**; the key stays in the node env /
  `lb-secrets`, never in a definition record (mirrors `profiles.rs`' `ModelEndpoint` and the shipped
  `agent.config`).
- **Not per-run model consumption of a per-workspace endpoint.** Actually routing the in-house loop to
  a *per-workspace* model endpoint at invoke time is gated on the **ai-gateway provider adapter** (the
  same dependency default-agent-wiring names: the in-house model is node-level `LB_AGENT_MODEL_*`
  today). This scope ships the catalog + seed + selection (which the invoke path already reads for the
  **runtime**); consuming the selected definition's **endpoint** per-workspace lands when a real
  `Provider` adapter + a per-run model override exist (named follow-up). Stated plainly so the UI
  doesn't over-promise.
- **Not per-user.** A definition and the active selection are **workspace** settings (like
  `workspace_prefs` / `agent.config`), never per-member.
- **No live feed / no batch / no job.** The catalog is a small config set read on demand; there is no
  motion to stream and no long-running bulk op (see MCP surface for why each is N/A).

## Intent / approach

**A catalog is a library of named `(runtime, endpoint)` presets; the active pick stays `agent.config`.**
Reuse three shipped patterns wholesale, add no new machinery:

1. **Storage + tiers = the core-skills split.** Custom definitions are workspace-scoped
   `agent_definition` records (the hard wall). Built-ins are seeded **once** into a reserved namespace
   `_lb_agents` (the `_lb_skills` / `_lb_identity` precedent) and are **read-only to users**
   (`builtin.*` ids reject `create/update/delete` with `Reserved → BadInput`, checked before the caps
   gate, exactly like `core.*` skills). `agent.def.list` returns the **union**: node-runnable built-ins
   ∪ the workspace's custom definitions.

2. **Seed = the core-skills boot seeder.** An embedded `agents.toml` manifest (`include_str!`,
   overridable by a node-config file path `LB_AGENT_CATALOG_TOML`) is parsed at boot and seeded into
   `_lb_agents` idempotently — the boot seeder is the **only** writer of that namespace, mirroring
   `seed_core_skills`. A node with the `external-agent` feature OFF still seeds the `open-interpreter.*`
   built-ins, but they are filtered from `list` because the node doesn't offer the runtime — the seed
   is symmetric; visibility is config (never an `if cloud`).

3. **Selection = the shipped `agent.config`.** Picking a definition writes `agent.config.set {
   default_runtime, model_endpoint }` from the definition's fields. The shipped
   `resolve_effective_runtime` (explicit → workspace default → registry default) already honors
   `default_runtime`, so **no new resolution path** — the catalog feeds the existing seam.

**Rejected alternatives.** (a) *Expanding `agent.config` into an array of endpoints* — rejected: the
built-ins need a node-level seed + read-only tier + node-upgrade re-seed, which a per-workspace array
can't give; the reserved-namespace pattern already solves all three. (b) *A brand-new "presets" verb
family unrelated to skills* — rejected: it would duplicate the exact `core.*`/user read-only-tier +
boot-seed machinery core-skills already ships; reuse it. (c) *Storing the selection as a copy of the
endpoint* — rejected in favor of the definition being the durable library and `agent.config` the pick;
a copy is fine for v1 (it feeds the existing `agent.config` fields) and a **reference** (`agent.config`
gains an optional `active_definition` id that re-resolves) is the named follow-up once per-workspace
endpoint consumption lands.

## How it fits the core

- **Tenancy / isolation:** custom `agent_definition` records are workspace-scoped (the hard wall) —
  a ws-B admin can never read/write/delete a ws-A custom definition. Built-ins live in the reserved
  `_lb_agents` namespace and are the same read-only union for every workspace (like core skills); they
  carry no tenant data. Every verb authorizes **workspace-first** via `lb_mcp::authorize_tool`.
- **Capabilities (the deny path):** member reads gate on `mcp:agent.def.list:call` /
  `mcp:agent.def.get:call`; admin writes on `mcp:agent.def.create:call` / `:update:call` /
  `:delete:call` (beside the shipped `mcp:agent.config.set:call`). Opaque deny. **Two extra walls on
  writes:** (1) a `builtin.*` id is `Reserved → BadInput` **before** the caps gate (read-only tier,
  regardless of caps); (2) a definition's `runtime` is validated against the node registry at write —
  an unrunnable id is `BadInput`, never a silent accept (the shipped `agent.config.set` rule).
- **Placement:** `either`. The seed + catalog run on every node (symmetric); which built-ins are
  *visible* is config (the `external-agent` feature gates whether `open-interpreter.*` appears). No
  `if cloud`.
- **MCP surface** (API shape §6.1) — **no new resource beyond the definition; CRUD + get/list only:**
  - **Read:** `agent.def.list` (the catalog the UI renders — node-runnable built-ins ∪ workspace
    custom, each tagged `builtin: true|false`); `agent.def.get {id}` (one entry). Both member-gated.
  - **Create/Update/Delete:** `agent.def.create {id, label, runtime, model_endpoint, …}`,
    `agent.def.update {id, patch}`, `agent.def.delete {id}` — admin-gated, **custom only** (built-ins
    rejected). One responsibility per file (FILE-LAYOUT): `catalog/{list,get,create,update,delete}.rs`.
  - **Consumes** the shipped `agent.runtimes` (to filter built-ins to node-runnable) and the shipped
    `agent.config.get/set` (selection). Gateway routes mirror the verbs 1:1 (`GET /agent/defs`, `GET
    /agent/defs/:id`, `POST/PATCH/DELETE /agent/defs[/:id]`), like `GET|PUT /agent/config`.
  - **Live feed: N/A** — a definition is config state, not motion; the UI re-reads `list` on save (no
    stream, per state-vs-motion §3 rule 3). **Batch: N/A** — a handful of small records, always fast,
    one per call (no bulk import, no long op, so no job). Said explicitly per §6.1.
- **Data (SurrealDB):** a SCHEMAFULL `agent_definition` table (composite id `[slug]`, LWW UPSERT —
  idempotent offline replay, exactly like `workspace_agent_config`), in the workspace namespace for
  custom entries and in the reserved `_lb_agents` namespace for built-ins. State only; no new datastore.
- **Bus (Zenoh): N/A** — no motion. Config changes are record writes the UI re-reads, not a feed.
- **Sync / authority:** node-local config records; the reserved built-ins are boot-seeded (hub or edge,
  symmetric). A custom definition written offline UPSERTs idempotently on reconnect (LWW on the slug).
- **Secrets:** `api_key_env` is an env NAME, mediated exactly as the shipped endpoint — never a value,
  never logged. The seed manifest carries only names.
- **No fake backend (rule 9):** real store (`mem://`), real caps, real gateway, real boot seed. The
  seed reads a real TOML into real records; tests seed built-ins through the real boot path and drive
  the verbs over the real gateway. No fake — there is no external here (the provider HTTP is not
  touched; a definition is names-only config).
- **SDK/WIT impact:** none — host + role wiring + a UI surface; no plugin-boundary change.

## UI (the Settings → Agent tab, extended)

The current Agent tab (runtime dropdown + one raw endpoint form) becomes a **catalog manager**:

- **Catalog list.** `agent.def.list` renders the pickable presets — built-ins (e.g. "In-house — Z.AI
  GLM-4.6", "Open Interpreter — Z.AI GLM-5.2") tagged read-only, plus the workspace's custom entries.
  Picking one is the workspace default: it writes `agent.config.set` with that definition's runtime +
  endpoint (admin only; a member sees the list + active pick read-only).
- **The active selection** is highlighted (resolved from `agent.config.default_runtime` +
  `model_endpoint`), replacing the "type a runtime + endpoint by hand" flow as the primary path.
- **Create / edit custom.** The existing raw endpoint form is repurposed as the **custom-definition
  editor** (`agent.def.create` / `agent.def.update`) — provider / model / api-key-env NAME / base URL +
  a label and a runtime dropdown (from `agent.runtimes`). Built-ins are non-editable (no edit/delete
  affordance); a `builtin.*` write is rejected server-side too (defense in depth).
- **Registry-drift handling** is reused: a definition whose runtime the node no longer offers is shown
  disabled with the shipped "not currently available" note, never silently dropped.
- **FILE-LAYOUT (frontend):** `features/settings/agent/` — `AgentCatalog.tsx` (the list/picker),
  `AgentDefinitionEditor.tsx` (create/edit custom), `useAgentCatalog.ts` (data), `agentDef.api.ts` (one
  call per verb, mirroring the backend). The existing `AgentTab.tsx` composes them.

## Example flow

1. A fresh node boots: `seed_agent_definitions` reads the embedded `agents.toml` and seeds six built-ins
   into `_lb_agents` (`builtin.in-house-glm-4.6/5.1/5.2`, `builtin.open-interpreter-glm-4.6/5.1/5.2`).
   Idempotent — a re-boot is a no-op; a node built without `external-agent` still seeds the
   open-interpreter entries (they just won't list).
2. Ada (admin) opens Settings → **Agent**. `agent.def.list` returns the node-runnable built-ins (the
   in-house three; the open-interpreter three only if the feature is on) — no custom entries yet.
3. Ada picks **"In-house — Z.AI GLM-4.6"**. The UI writes `agent.config.set { default_runtime:
   "default", model_endpoint: {provider:"zaicoding", model:"glm-4.6", api_key_env:"ZAI_API_KEY",
   base_url:"…/coding/paas/v4"} }`. Every `agent.invoke` that omits `runtime` now resolves to `default`
   via the shipped `resolve_effective_runtime`.
4. Ada adds a custom **"In-house — GLM-5.2 (staging key)"**: `agent.def.create { id:"staging-glm-5.2",
   label, runtime:"default", model_endpoint:{…, api_key_env:"ZAI_STAGING_KEY"} }` → a workspace-scoped
   record. It appears in the catalog; she picks it.
5. Bob (member, no admin cap) opens the tab: he sees the catalog + the active pick **read-only** — a
   create/update/delete is opaquely `Denied`.
6. Ada tries to edit **"In-house — Z.AI GLM-4.6"** (a built-in): `agent.def.update { id:
   "builtin.in-house-glm-4.6" }` → `Reserved → BadInput` (read-only tier), even though she is admin.
7. A ws-B admin `agent.def.get`/`delete`s Ada's `staging-glm-5.2` → not found / denied: the custom
   catalog is workspace-walled.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), all rule-9 (real store `mem://` / caps /
gateway / boot seed; no fake — a definition is names-only config, no external HTTP):

- **Capability-deny, per verb (§2.1):** each of `list/get/create/update/delete` denied without its cap;
  a member (no admin caps) is denied every write while `list/get` succeed.
- **Read-only built-in tier:** `create`/`update`/`delete` of a `builtin.*` id is `Reserved → BadInput`
  regardless of caps (checked before the caps gate) — the core-skills read-only proof, mirrored.
- **Workspace-isolation (§2.2):** a ws-B admin cannot `get`/`update`/`delete` a ws-A **custom**
  definition; built-ins are the same read-only union for both, carry no tenant data.
- **Seed idempotency + node-runnable filter:** boot seeds the six built-ins into `_lb_agents`; a second
  boot is a no-op (idempotent); with `external-agent` OFF, `agent.def.list` **omits** the
  `open-interpreter.*` built-ins (runtime not offered) while still seeding them; feature ON lists them.
- **Runtime validation on write:** `create`/`update` with a `runtime` the node doesn't offer is
  `BadInput` (the shipped `agent.config.set` rule), never a silent accept.
- **Selection round-trip:** picking a definition writes `agent.config` such that a `runtime`-omitted
  `agent.invoke` resolves to the definition's runtime (compose with the shipped
  `resolve_effective_runtime` / `agent_default_runtime_test`).
- **MCP roundtrip + gateway routes:** each verb over the real gateway (`/agent/defs*`), plus a per-verb
  route deny (mirroring `tools_catalog` / `agent.config` route tests).
- **UI (Vitest against a real in-process gateway, seeded — no `*.fake.ts`):** the catalog renders the
  seeded built-ins + a seeded custom entry; a member sees it read-only; an admin create/edit/delete
  drives the real verbs; a built-in has no edit/delete affordance; picking sets the active selection.
- **Offline/sync:** a custom definition written twice (double-delivery) UPSERTs idempotently (LWW on
  the slug) — the `workspace_agent_config` precedent.

## Risks & hard problems

- **Model ids drift.** `glm-5.1` / `glm-5.2` are the provider's ids as the user named them; if Z.AI
  publishes different ids the manifest is a one-line TOML edit + re-seed (no code change). Keep the
  model id **data in the TOML**, never hard-coded in a verb.
- **The endpoint isn't consumed per-workspace yet.** Picking a definition sets `agent.config`, and the
  invoke path honors the **runtime** today — but the in-house **model** is still node-level
  (`LB_AGENT_MODEL_*`) until the ai-gateway provider adapter lands. The UI must not imply "this key/model
  is live per workspace" beyond what's wired. Copy this honestly (a small "applies to routing once a
  provider adapter is configured" note), mirroring default-agent-wiring's honesty.
- **Built-in vs custom id collisions.** A custom id must not shadow a `builtin.*` id — reserve the
  `builtin.` prefix (reject it in `create`, like `core.` for skills) so the two tiers can't collide.
- **Node-upgrade re-seed semantics.** New built-ins on upgrade must add without clobbering a workspace's
  active pick; the pick lives in `agent.config` (a copy of the fields), so a re-seed of the *library*
  never disturbs the *selection* — verify this composition in a test.

## Open questions

- **Manifest location & override.** Embed `agents.toml` in the host crate
  (`crates/host/src/agent/catalog/builtins.toml`, `include_str!`) with a node-config file override
  (`LB_AGENT_CATALOG_TOML`)? Proposal: yes — the core-skills embed precedent, plus one override path so
  an operator can extend the built-ins without a rebuild.
- **Built-in versioning.** Version built-ins like core skills (`builtin.<id>@<node-version>`, keep old
  for rollback) or overwrite in place? Proposal: overwrite in place (a definition is small config, not
  a code artifact needing rollback) — simpler; revisit if a bad seed needs pinning.
- **Selection: copy vs reference.** v1 copies the definition's fields into `agent.config` (feeds the
  shipped resolution). Add `agent.config.active_definition` (a reference that re-resolves) when
  per-workspace endpoint consumption lands? Proposal: copy now, reference as the named follow-up.
- **Do we keep the raw endpoint form as an "unsaved custom"?** Or force every endpoint through a named
  definition? Proposal: every endpoint is a named custom definition (cleaner management, one model) —
  the raw form becomes the create-custom editor.
- **`description`/labels on built-ins** — carry human labels + a short description in the TOML
  (recommended, so the picker reads well) vs. derive from id? Proposal: carry them in the TOML.

## Related

- `agent/agent-catalog-test-and-secrets-scope.md` (the follow-up: a `agent.def.test` "does it have
  MCP/ACP/skills context" button + a DB-sealed per-workspace model key via `lb-secrets`, replacing the
  names-only-env-var-only key with a sealed secret path — extends this catalog).
- `agent/default-agent-wiring-scope.md` (the finished in-house default this catalog picks + feeds; the
  `LB_AGENT_MODEL_*` node-level model + the ai-gateway-adapter dependency the per-workspace-endpoint
  follow-up waits on), `agent/agent-scope.md` (the engine).
- `external-agent/agent-config-scope.md` (the shipped one-selection `agent.config` this catalog feeds),
  `external-agent/agent-runtimes-scope.md` (`agent.runtimes`, consumed to filter built-ins),
  `external-agent/runtime-seam-scope.md` / `external-agent/external-agent-scope.md` (the runtimes a
  definition binds; `profiles.rs`' Z.AI `zaicoding` endpoint the seed reuses),
  `external-agent/model-routing-scope.md` (#4 — the served face the per-workspace-endpoint follow-up
  relates to).
- Core-skills (the seed + reserved-namespace + read-only-tier pattern this reuses): `public/skills/skills.md`,
  `seed_core_skills` / `CORE_SKILLS_NS` in `lb-assets`.
- Skills: the new/updated `../../skills/agent/SKILL.md` (the implementing session extends it to drive the
  catalog: list, create a custom definition, pick it, watch a run use it).
- README `§6.16` (shared AI agents), `§6.14`/`§6.15` (gateway), `§6.7` (secrets), `§7` (tenancy),
  `§3` (rules 1/5/6).
- Code the build will touch: `crates/host/src/agent/catalog/*` (new — record, seed, the five verbs),
  `crates/host/src/agent/config/*` (selection integration), `crates/host/src/tool_call.rs` (dispatch the
  `agent.def.*` verbs under the `agent.` branch), `node/src/main.rs` (call `seed_agent_definitions` at
  boot, beside `seed_core_skills`), `role/gateway/src/routes/*` (the `/agent/defs*` routes),
  `ui/src/features/settings/agent/*` + `AgentTab.tsx`, `ui/src/lib/agent/*`.
