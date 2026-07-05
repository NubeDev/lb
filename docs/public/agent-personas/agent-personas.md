# Agent personas — shipped truth

A **persona** is a workspace-selected *focus* for the agent: `{ identity, granted_tools,
grounding_skills, extends }`, applied at run assembly to **narrow** a run — a curated tool subset, a
pinned grounding-skill set, and an identity prompt. It is **data, never code** (rule 10), and it
**narrows, never widens** the capability wall.

The problem it solves (observed with live external-agent use): a run was handed *everything* — the
constant `"You are a workspace agent."` prompt, the caller's whole reachable tool catalog, no task
grounding — and the agent was confused. A persona replaces the everything-menu with a focus.

> **Status.** **#1 (persona-model) — SHIPPED** (record, CRUD, selection, run-assembly for both
> runtimes, `agent.persona.resolve` / `agent.policy.get`, Settings UI — 6 gateway tests green).
> **#2 (grounding corpus) — SHIPPED.** **#3 (built-in catalog) — SHIPPED** (7 personas + confusion
> demo). #4 (extension-builder persona) builds on them.
> Scope: [`scope/agent-personas/`](../../scope/agent-personas/agent-personas-scope.md).
> Sessions: [persona-model](../../sessions/agent-personas/persona-model-session.md) ·
> [persona-grounding](../../sessions/agent-personas/persona-grounding-session.md) ·
> [persona-catalog](../../sessions/agent-personas/persona-catalog-session.md).

## The load-bearing rule

**Effective menu = persona ∩ agent ∩ caller; the wall re-checks every call.** A persona's
`granted_tools` filter the run's *advertised* tools — the in-house model's proposable menu AND the
external ACP bridge's advertised set. It is a hint, not a grant:

- a persona listing a tool the caller lacks → still denied at `caps::check` (never in the reachable
  menu to begin with);
- a granted tool the persona omits → un-advertised, but a model that proposes it anyway still hits the
  unchanged wall.

Picking a persona never grants a capability. Admins keep granting caps exactly as before; the persona
narrows *within* grants. (Rejected: personas-as-grants — it would couple entitlement to focus.)

## The record (two tiers, one shape)

| Tier | Where | Writable |
|---|---|---|
| **built-in** (`builtin.<slug>`) | reserved `_lb_personas` namespace, seeded from `personas.toml` | read-only (a `builtin.*` write is `BadInput` before the caps gate) |
| **custom** (ws slug) | the workspace namespace | admin CRUD |

```
Persona {
  id, label, description?,
  identity,                 // prepended to the in-house SYSTEM_PROMPT / folded at the head of the goal
  granted_tools: [..],      // tool ids or trailing-* globs — OPAQUE data (rule 10); a narrowing hint
  grounding_skills: [..],   // skill ids pinned at session start (grant-gated, FAIL-CLOSED)
  extends: [..],            // parent persona ids; tool/skill lists union at read (child identity wins)
  policy_preset?, runtimes?,// #4: an Allow/Ask/Deny supervision floor + a runtime restriction
}
```

Built-ins live once (node-level, reserved namespace, readable everywhere, writable nowhere — the
boot seeder is the only writer); custom personas are workspace-walled (a ws-B run can never resolve a
ws-A custom persona).

## MCP surface

| Verb | Tier | Shape |
|---|---|---|
| `agent.persona.list` | member | `{} → { personas }` |
| `agent.persona.get` | member | `{ id } → { persona }` |
| `agent.persona.resolve` | member | `{ id? } → { effective }` — the extends-unioned effective persona (the Settings "effective tools" view; `id?` absent → the active persona) |
| `agent.persona.create` | admin | `<Persona> → { ok }` (custom only) |
| `agent.persona.update` | admin | `{ id, patch } → { ok }` (a present list REPLACES) |
| `agent.persona.delete` | admin | `{ id } → { ok }` |
| `agent.policy.get` | member | `{} → { rules }` — read the Allow/Ask/Deny policy (its first read verb) |
| `agent.policy.set` | admin | `{ rules } → { ok, rules: N }` (shipped since agent-run Part 2) |

Caps: `mcp:agent.persona.<verb>:call` + `mcp:agent.policy.get:call`. Selection: `agent.config` gained
an additive optional `active_persona` (the `active_definition` move); `agent.invoke` gained a per-run
`persona` override on every front door (channel payload, routed request, `POST /agent/invoke`).

## Run assembly — one seam, both runtimes

At `invoke_via_runtime` (the seam the in-house loop and the external ACP runtime share):

1. **resolve** the persona (explicit invoke arg > `active_persona` > none; explicit-unknown = named
   error; dangling active = warn + un-narrowed) and union its `extends` closure;
2. **runtime restriction** (#4): a persona may pin `runtimes` — a disallowed pairing fails at start
   with a named error, before any model spend;
3. **narrow** the menu to `reachable ∩ granted_tools` (glob = trailing-`*` prefix);
4. **identity + pinned bodies** fold into the goal (reaches both runtimes — the goal seeds the
   in-house rehydrate and is the external agent's only channel); **fail-closed** — an ungranted pinned
   skill fails the run at start with the named `PersonaSkill` error;
5. **catalog** is filtered to the pinned skill set (the model sees the persona's focus). The grant
   stays the wall — filtering only removes already-granted entries.

Resolution at run assembly reads the persona via a **raw, namespace-walled store read**, deliberately
NOT gated on `mcp:agent.persona.get` — a persona read can only narrow, so gating it would guard
nothing while breaking the common case (a member whose workspace picked a persona must have it apply).
The CRUD *verbs* keep their cap gate for the Settings surface.

## Selection & precedence

Explicit per-invoke `persona` arg → workspace `agent.config.active_persona` → none (un-narrowed).
A stored persona that no longer resolves → registry-default (un-narrowed) behavior + `warn!`, never an
errored run. Personas are **orthogonal to definitions**: the same persona runs on the in-house or an
external runtime — persona picks *focus*, the definition picks *(runtime, model)*.

## Grounding (#2) — the agent learns from docs, not the codebase

A persona's `grounding_skills` pin real, granted, versioned skills whose **bodies** load into the run's
context at session start. The corpus is the platform's own operating knowledge, seeded from the repo:

- **The whole `docs/skills/` tree, dynamically** — the `lb-assets` build script scans every
  `docs/skills/<name>/SKILL.md` (no allow-list; a new dir auto-seeds as `core.<name>`).
- **The `docs/testing/**` e2e runbooks** — each frontmatter-bearing runbook seeds as `core.e2e-*`
  (`core.e2e-backend`, `core.e2e-frontend`, `core.e2e-{nav,system,dashboard,charts,datasources}`),
  pulled from where the docs live (no copy that can drift). The frontmatter-less README index is skipped.
- **Three authored grounding skills:** `core.mcp` (the MCP contract), `core.acp` (the ACP surfaces,
  honest about partial ACP v1), `core.extension-authoring` (the devkit developer manual, #4's grounding).

**Anti-rot gate:** a `docs/skills/` subdir missing its `SKILL.md` now **fails the build** — a
half-authored skill can never silently ship as "absent" (which would fail-close a persona that pins it
at run time). The seeded count equals the on-disk corpus (currently 34); don't hardcode it.

**Grounding is grant-gated, fail-closed:** pinning `≠` granting. A persona pins a skill; the workspace
must have granted it (`grant:skill/{id}`) or the run fails at start with the named error. Proven: a
persona-grounded run answers "how do I verify this feature?" from the `core.e2e-backend` runbook body in
its context, with no filesystem/repo tool in its menu.

## The built-in catalog (#3)

Seven built-in personas ship as `personas.toml` data (read-only in `_lb_personas`), each curating one
platform area's verbs + pinning ≤ 4 grounding skills:

`builtin.data-analyst` · `builtin.flow-author` · `builtin.widget-builder` · `builtin.rules-author`
(extends flow-author + data-analyst) · `builtin.workspace-admin` · `builtin.channels-operator` ·
`builtin.system-manager` (extends all six; the general operator that hands off deep work).

Two deliberate stances: **destructive/security verbs (`workspace.delete/purge`, `authz.revoke-tokens`,
`secret.get`) are excluded from every persona** (advertising a catastrophic verb to a model invites a
catastrophic proposal; a human runs those), and **`system-manager`/`rules-author` are `extends`-composed,
not hand-flattened** (the union stays current as parents evolve).

**What "narrow the menu" reaches today (a recorded finding):** the run's menu is the palette-descriptor
catalog (`tools.catalog` = `host_descriptors()` ∩ caps) + loaded extension tools — a curated subset, not
the full ~175-verb surface. A persona's `granted_tools` list is the complete **forward-looking**
allow-list (it narrows verbs correctly as they gain descriptors / arrive as extension tools). On a bare
node the tool-menu is already small, so **identity + pinned grounding** carry most of the confusion cure
there; tool-narrowing bites hardest with many extension tools loaded. See
`scope/agent-personas/persona-catalog-scope.md` → "Implementation finding".

**The confusion fix, demonstrated:** the same task, same caller — the reachable palette narrows from 11
tools to 1 under `builtin.data-analyst` (off-task palette tools gone, the on-task `federation.query`
stays), and the run is grounded in the data skills. Proven in `agent_persona_catalog_test.rs`.

## What it does NOT do

- No new capability, no widening — the wall (`caps::check`) is untouched by this entire topic.
- No persona-partitioned agent memory (memory stays workspace + member scoped — a persona switch must
  not amnesia the workspace).
- No per-persona model/budget (model rides the definition).

## Related

- Scope: [`persona-model-scope.md`](../../scope/agent-personas/persona-model-scope.md),
  [umbrella](../../scope/agent-personas/agent-personas-scope.md).
- Skill (operating manual): `docs/skills/agent/SKILL.md` §7 "Personas".
- Substrate it rides: `public/skills/skills.md` (grant gate + catalog inject), the agent-definition
  catalog (record/tier/seed pattern), `agent.config` (the selection pointer).
