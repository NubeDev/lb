# Agent setup wizard — session

Status: shipped. Added a "Set up the agent" wizard to the Setup tab that walks a user through the
workspace AI agent with a plain-language intro of what each part is for, then one page per key part.

## The ask

From the Settings › Agent tab (definition catalog + persona/tools/permissions), build a guided Setup
wizard: intro the agent + a page per key part, explaining what each is used for. See
`docs/scope/agent/*` and `docs/scope/admin/setup/setup-wizards-scope.md`.

## What the agent is (the mental model the wizard teaches)

An agent run = **who runs × what for**, bounded by what it can reach and how it's supervised:

- **Definition — who runs.** A named runtime + model preset (e.g. in-house loop over Z.AI GLM-4.5).
- **Persona — what for.** A focused job: curated tool menu, pinned skills, identity.
- **Tools — what it can reach.** The live `persona ∩ agent ∩ caller` set (read-only boundary view).
- **Permissions — how it's supervised.** Per-tool Allow / Ask / Deny (tightens, never grants).

## Steps

1. **Overview** — pure copy: the mental model + the four-part list (new glue only).
2. **Definition** — the real definition catalog + active pick.
3. **Persona** — the real persona section (roster + Effective-tools + Permissions in one).

## Reuse — extraction, not fork (setup rule 3)

`AgentTab` bundled the definition-catalog block inline. To share that exact editor with the wizard
without forking its `useAgentCatalog` wiring, I **extracted** the block into
`settings/agent/AgentCatalogSection.tsx` and rewired `AgentTab` to render it. Now the Settings tab and
the wizard render one component. The persona half (`PersonaSection`) was already self-contained and
drops in as-is; it already lays out the roster, the Effective-tools view, and the Permissions pane as
labelled sub-sections, so the "Tools" and "Permissions" key parts are explained in the intro and shown
in-context on the Persona page rather than forked into separate editors.

## Reuse ledger

| Step | Reused from (component / hook / verb) | New code written? |
|---|---|---|
| Overview | — (pure explanatory copy) | intro copy in `AgentWizard.tsx` only |
| Definition | `settings/agent/AgentCatalogSection` (extracted from `AgentTab`) → `useAgentCatalog`, `AgentCatalog`, `AgentDefinitionEditor`; verbs `agent.def.*`, `agent.config.*`, `agent.runtimes` | extraction only (rule 3), no new editor |
| Persona | `settings/agent/PersonaSection` verbatim → `PersonaCatalog`, `EffectiveTools`, `PolicyPane`; verbs `agent.persona.*`, `agent.policy.*`, `prefs.*` | none |

No new backend, no new verb, no duplicated editor. Cap-gating hides controls (rule 5 — the gateway is
the wall); ids stay opaque (rule 10 — no branching on an extension/definition/persona id).

## Files touched

- `ui/src/features/settings/agent/AgentCatalogSection.tsx` — **new** (extracted from `AgentTab`).
- `ui/src/features/settings/AgentTab.tsx` — now composes `AgentCatalogSection` + `PersonaSection`.
- `ui/src/features/admin/setup/AgentWizard.tsx` — **new** wizard (intro + Definition + Persona).
- `ui/src/features/admin/setup/AgentWizard.gateway.test.tsx` — **new** real-gateway test.
- `ui/src/features/admin/setup/catalog.ts` — added the `agent` entry (`Bot` icon).
- `ui/src/features/admin/setup/SetupHub.tsx` — added the `agent` branch.

## Tests (real gateway, no fakes — CLAUDE §9)

`AgentWizard.gateway.test.tsx` drives the wizard against a real seeded gateway: asserts the intro names
the four parts, then reuses the real seeded catalog to pick an active definition and **reads the effect
back** (`getAgentConfig().default_runtime`), then advances to the real persona page.

```
✓ src/features/admin/setup/AgentWizard.gateway.test.tsx (1 test)         # new
✓ src/features/settings/AgentCatalog.gateway.test.tsx (3 tests)          # extraction preserved the tab
```

`npx tsc --noEmit` clean. Cap-deny + workspace-isolation are already exercised by the reused editors'
own gateway tests (`AgentCatalog.gateway.test.tsx` covers member read-only vs admin write; a fresh
`nextWs()` per test isolates the shared node).
