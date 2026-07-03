# Agent-catalog build session

Scope: `docs/scope/agent/agent-catalog-scope.md`. Built on `master`.

## What shipped

A catalog of named agent definitions — `(runtime, model_endpoint)` presets — in two tiers over one
record shape, reusing the shipped core-skills seed + reserved-namespace + read-only-tier machinery
wholesale (no new machinery):

- **Built-ins** — six presets boot-seeded from an embedded `agents.toml` manifest into the reserved
  `_lb_agents` namespace, read-only to users. In-house (runtime `default`) and Open Interpreter
  (runtime `open-interpreter-default`) × Z.AI GLM-4.6 / 5.1 / 5.2, over the `zaicoding` coding endpoint
  (`ZAI_API_KEY`). Names only — no secret values in the manifest or a record.
- **Custom** — workspace-scoped `agent_definition` records with full admin CRUD (custom-only writes).

### Backend (`crates/host/src/agent/defs/`)

- `model.rs` — `AgentDefinition` / `DefinitionEndpoint`, the `builtin.` reserved-prefix rule
  (`is_builtin`), the `kind` discriminator for the list filter.
- `store.rs` — the `agent_definition` table + raw get/list/upsert/delete over a namespace
  (`AGENT_DEFS_NS = _lb_agents`). `builtin` is derived from the namespace on read, never stored/trusted.
- `seed.rs` — `seed_agent_definitions`: parses the embedded `agents.toml` (`include_str!`, overridable
  by `LB_AGENT_CATALOG_TOML`), stamps `builtin.` ids, idempotent LWW UPSERT. The only writer of
  `_lb_agents`.
- `validate.rs` — the two write walls: `reject_reserved` (a `builtin.*` id → `BadInput`, **before** the
  caps gate) and `validate_runtime` (a runtime the node can't run → `BadInput`).
- `list/get/create/update/delete.rs` — the five gated verbs (`agent.def.*`), one per file.
  `list` = node-runnable built-ins ∪ workspace custom.
- `tool.rs` — the MCP bridge (`call_agent_catalog_tool`), dispatched from `agent/tool.rs`'s
  `agent.def.` branch.
- Boot: `node/src/main.rs` calls `seed_agent_definitions` beside `seed_core_skills`; the
  `test_gateway` bin does the same so the UI catalog test reads real seeded records.
- Gateway: `role/gateway/src/routes/agent_defs.rs` — `/agent/defs` (GET list, POST create),
  `/agent/defs/{id}` (GET get, PATCH update, DELETE delete). Dev-login caps extended with the five
  `agent.def.*` caps.

### Selection (no new resolution seam)

Picking a definition writes the shipped `agent.config.set { default_runtime, model_endpoint }` from the
definition's fields, so `resolve_effective_runtime` honors it unchanged. The catalog is the library;
`agent.config` stays the one active selection.

### UI (`ui/src/features/settings/agent/` + `lib/agent/agentDef.api.ts`)

- `agentDef.api.ts` — one call per verb over the MCP bridge.
- `useAgentCatalog.ts` — loads catalog + runtimes + active config; exposes pick/create/update/remove.
- `AgentCatalog.tsx` — the list/picker: built-ins read-only (no edit/delete affordance), custom
  editable, active pick highlighted, registry-drift disabled-with-note reused.
- `AgentDefinitionEditor.tsx` — the repurposed raw endpoint form as the custom-definition editor.
- `AgentTab.tsx` — composes them; honest copy ("a per-workspace model endpoint applies once a provider
  adapter is configured") per the scope's Risks.

## Honest call carried from the scope

Picking sets the workspace default **runtime** today (the invoke path honors it); routing the in-house
loop to a **per-workspace endpoint** is still gated on the ai-gateway provider adapter
(`default-agent-wiring`). The UI copy does not over-promise. Named follow-up: `agent.config`'s
`active_definition` reference (the copy-vs-reference open question) once per-workspace endpoint
consumption lands.

## Tests (all real store / caps / gateway / boot seed — rule 9, no fakes)

- Rust `crates/host/tests/agent_defs_test.rs` (8): seed + node-runnable filter (open-interpreter
  seeded but filtered), seed idempotency, per-verb capability-deny, read-only built-in tier
  (`BadInput` before caps even for an admin), custom CRUD round-trip, runtime validation on write,
  workspace-isolation, double-create idempotency (LWW), names-only.
- UI gateway (real seeded gateway, no `*.fake.ts`): `AgentCatalog.gateway.test.tsx` (3) — seeded
  built-ins list + open-interpreter filtered, admin create/edit/pick/delete + built-in read-only,
  member read-only. Updated `SettingsView.gateway.test.tsx` and `AgentDefaultRuntime.gateway.test.tsx`
  to the catalog UI.

## Notes / limitations found while building

- The copy-based active-selection resolves by `(runtime, provider, model)`; two definitions with
  identical resolved fields tie (the first wins). Documented as the copy-vs-reference open question;
  the reference follow-up removes the ambiguity. Surfaced in a test comment.
