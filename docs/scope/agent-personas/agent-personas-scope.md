# Agent-personas scope — overview & index

Status: scope (the ask, umbrella). Promotes to `public/agent-personas/` once the slices ship.

The agent works, but a run today is handed **everything**: the constant `"You are a workspace
agent."` system prompt, the caller's *entire* reachable tool catalog as its menu, and no
task-specific grounding. The observed symptom (real, from live external-agent use): the agent is
**confused** — too many tools, no identity, no idea which platform docs govern the task. The fix
the platform already half-designed: the external-agent umbrella's thesis that *"what it can do =
the granted tools, who it is = the granted persona skill… a profile decision, not code."* This
topic **productizes that as a user-facing choice**: a workspace picks what it wants the agent
*for* — data analysis, flow authoring, widget building, rules, admin, channels, extension
coding, or general management — and the run is assembled from that **persona**: a curated tool
subset, a pinned grounding-skill set, and a persona identity prompt. All of it **data, never
code** (rule 10), and all of it **narrowing, never widening** the capability wall.

Broken into **four sub-scopes** (plus a follow-up correction, #5), each independently reviewable and
shippable, with one umbrella exit gate. This doc is the index, the thesis, and the cross-cutting rules.

> **Follow-up (post-ship correction).** #1 shipped "active persona" as a single mutable
> `agent.config.active_persona` record — a **workspace-global toggle**. Live use showed that is wrong
> twice: wrong scope (two members / two tabs stomp each other) and wrong interaction (the dock already
> knows *where the user is*, so a hand-picked global persona is backwards).
> [persona-session](persona-session-scope.md) (#5) replaces it: the workspace **enables a roster**
> (`agent.config.enabled_personas`, `None` = all); each run applies **exactly one** persona, suggested
> client-side from the page context (`Persona.surfaces` — data, rule 10) with a sticky per-tab pin,
> sent as the shipped per-invoke `persona` arg; stored *defaults* move to a `Prefs.agent_persona` axis
> (member → workspace-default fold). Runtime union of N personas was **rejected** (identity prose
> doesn't union — that's the confusion again); "many at once" stays `extends` composition as a record.
> Reuses #1's record and run-assembly seam unchanged.

> **Why now.** `agent-close-out-scope.md` deferred the "curated/bounded tool subset" as *"a
> solution without a symptom."* The symptom has arrived (a confused external agent over the full
> surface), so the deferral is reversed — and widened from a menu-trim into the persona system,
> because the confusion has three causes (menu, identity, grounding), not one.

## The thesis (read once, applies to every sub-scope)

**A persona = `{ granted_tools, grounding_skills, identity }` — a bundle of already-shipped
grant-gated data, selected per workspace, applied at run assembly.** Nothing new crosses the wall:

1. **The menu narrows; the wall doesn't move.** A persona's `granted_tools` filters
   `reachable_tools` (the run's `AllowedTool` menu) — it is a *narrowing hint over* the shipped
   `agent ∩ caller` intersection, never a grant. Every dispatch still re-runs `caps::check`; a
   persona listing a tool the caller lacks changes nothing (deny). Effective menu =
   **persona ∩ agent ∩ caller**.
2. **Grounding = pinned granted skills.** The runtime skills system (two-tier catalog,
   `_lb_skills` seed, `grant:skill/{id}` gate, `render_catalog` inject / external goal-fold,
   `load_substrate_skill`) is shipped end to end. A persona *pins* a skill set: identities +
   operating manuals the agent is grounded with at session start instead of discovering the whole
   catalog. A persona whose skill isn't granted **fails the run at start** (fail-closed — the
   acp-driver decision, kept).
3. **Identity = the persona skill's voice.** The in-house `SYSTEM_PROMPT` constant gains a
   persona prepend; the external runtime bakes the persona-skill body into the goal via the
   *shipped* substrate fold. Same content, both runtimes, one source.
4. **Selection rides the shipped catalog.** A persona is picked the way an agent definition is
   picked — a workspace-level choice on `agent.config`, plus an optional per-invoke override.
   Built-in personas seed read-only (the `_lb_agents` / core-skills pattern, third reuse);
   custom personas are workspace CRUD.

**Rejected: personas as code (a match on persona id in the loop).** Rule 10 in miniature: the
run-assembly seams treat the persona id as opaque data resolving to records; `if persona ==
"data-analyst"` anywhere in host code is the leak. A new persona must be creatable **as a record
with zero code change** — that is the swap test.

**Rejected: personas as capability grants.** Tempting (one mechanism), wrong: a persona is a
*focus*, chosen freely per task, possibly per message; a grant is an *entitlement*, admin-given.
Coupling them would either let picking a persona widen caps (a hole) or make caps churn on every
persona switch (unusable). The persona narrows *within* grants; admins keep granting caps exactly
as today.

**Rejected: one mega-scope.** The four concerns rot at different speeds (the record shape is
stable; the built-in catalog will grow per feature; grounding tracks the docs corpus; the coding
persona has its own safety posture). Sub-scopes keep each reviewable.

## Architecture map

```
Settings → Agent → persona picker            current pick = agent.invoke { persona } (client/tab)  [persona-session #5]
        │ default = Prefs.agent_persona (member → workspace-default fold)
        ▼
persona:{id} record (built-in seed | workspace custom)      [persona-model]
   { granted_tools, grounding_skills, identity, extends? }
        │ resolved at run assembly (dispatch.rs / run.rs — the ONE seam both doors share)
        ▼
 ┌────────────────────────────── run context ──────────────────────────────┐
 │ menu   = reachable_tools ∩ persona.granted_tools   (wall re-checks all) │
 │ prompt = SYSTEM_PROMPT + persona identity (in-house)                    │
 │          goal-fold of persona-skill body (external, shipped substrate)  │
 │ skills = render_catalog pinned to persona.grounding_skills             │
 └──────────────────────────────────────────────────────────────────────────┘
   built-in persona catalog: data-analyst, flow-author, widget-builder,     [persona-catalog]
   rules-author, workspace-admin, channels-operator, system-manager
   grounding corpus: docs/skills + docs/testing → seeded core skills        [persona-grounding]
   extension-builder persona (UI/WASM/process, never free coding)           [persona-coding]
```

## The four sub-scopes (build order)

| # | Sub-scope | Owns | Depends on |
|---|---|---|---|
| 1 | [persona-model](persona-model-scope.md) | The persona record (shape, two tiers, CRUD verbs, `extends` composition), selection (`agent.config.active_persona` + per-invoke override), and the **run-assembly application** on the one shared seam (menu narrowing, identity prepend/fold, skill pinning) — for BOTH runtimes. The foundation. | shipped: skills, agent catalog, `agent.config`, `render_catalog`, substrate fold |
| 2 | [persona-grounding](persona-grounding-scope.md) | The grounding corpus: promote the platform's own operating knowledge (`docs/testing/` runbooks; the MCP/ACP/extension-authoring skills) into seeded, grantable core skills a persona can pin — so the agent learns the platform from **docs, not from reading the whole codebase**. | #1 (pins), shipped core-skills seed |
| 3 | [persona-catalog](persona-catalog-scope.md) | The built-in personas as **data** (a `personas.toml` seed): data-analyst, flow-author, widget-builder, rules-author (composes flow+data via `extends`), workspace-admin, channels-operator, system-manager — each with its exact verb allow-list and pinned skills. | #1 (record), #2 (skills to pin), the MCP verb inventory |
| 4 | [persona-coding](persona-coding-scope.md) | The **extension-builder** persona: the agent codes UI/WASM/process **extensions** against the devkit, in a scoped workdir, driven — *"100% coding, but never on its own."* The persona with a safety posture of its own. | #1–#3, `scope/extensions/`, external-agent capability-wall for the sandbox story |
| 5 | [persona-session](persona-session-scope.md) | **Correction of #1's selection.** Workspace enables a **roster** (`enabled_personas`, `None` = all); each run applies **one** persona — context-suggested from the page (`Persona.surfaces`, matched client-side over the roster) with a sticky per-tab pin, sent via the shipped per-invoke `persona` arg; defaults move to a `Prefs.agent_persona` axis (member → ws-default fold). Union-of-N rejected — `extends` records stay the composition path. Zero new verbs. Fixes concurrent members / multi-tab stomping + "why must I hand-pick?". | #1 (record, override seam, `resolve_persona`), #3 (`personas.toml` gains `surfaces`), `scope/prefs/`, agent-dock context |

Ship order: **#1 → #2 → #3 → #4**; **#5** is a post-ship correction, shippable any time after #1.
#1 alone already fixes the observed confusion for a hand-authored persona; #3 makes it a product;
#4 is the persona that needs the most care; #5 makes the pick correct under concurrent sessions.

## Cross-cutting platform checklist (addressed topic-wide; sub-scopes carry the detail)

- **Workspace is the hard wall** — persona records are workspace-scoped (custom) or reserved-
  namespace read-only (built-in); the pick lives on the ws-scoped `agent.config`; a ws-B run can
  never resolve ws-A's persona or its pinned skills (isolation test in #1).
- **Capability-first** — personas **narrow, never widen**: effective menu = persona ∩ agent ∩
  caller, every call re-checked; persona CRUD gets its own caps (member read / admin write, #1);
  an ungranted pinned skill fail-closes the run.
- **Symmetric nodes** — persona resolution is data + config on the shared dispatch seam; no role
  branch anywhere.
- **One datastore** — persona records in SurrealDB (the `_lb_agents`-pattern seed + workspace
  CRUD); no new store.
- **State vs motion** — a persona is pure state read at run assembly; nothing of it rides the bus.
- **Stateless extensions** — untouched; personas live in host records, not extension instances.
- **MCP is the contract** — persona CRUD/selection are MCP verbs (#1); the tools a persona curates
  are MCP tool ids treated as **opaque strings** (rule 10 — a persona naming `github.pr.open` is
  data, exactly like the outbox `Target`).
- **No mocks (rule 9)** — every test seeds real persona records into the real store and runs the
  real loop/gateway; the only fake stays the provider HTTP (`MockProvider`).
- **Agent-memory** — memory stays **workspace + member scoped, NOT persona-partitioned** (keeps
  the agent-memory decided posture; a persona switch must not amnesia the workspace). Revisit only
  with a real cross-persona-contamination symptom; recorded as the deliberate alternative.
- **SDK/WIT impact** — none; nothing crosses the guest ABI.
- **Skill docs** — `skills/agent/SKILL.md` gains the persona how-to (#1 session); #2 *creates*
  skills as its deliverable; each built-in persona's pinned set is listed in #3.

## Umbrella exit gate — **ALL FIVE MET (2026-07-05)**

The topic is shippable when — and it is:

- ✅ **The swap test (#1):** a brand-new persona created **as a record only** (custom CRUD, zero code
  change) drives a run whose menu, identity, and grounding all reflect it — proven for **both**
  runtimes (in-house via a recording model; external via a scripted `AgentRuntime` capturing its
  `RunContext` — the narrowed `tools` are exactly what the real ACP bridge advertises).
  `agent_persona_test.rs::swap_test_in_house` + `::swap_test_external`.
- ✅ **The narrowing test (#1):** a persona listing a tool the caller lacks is never added to the menu
  (`narrowing_a_persona_tool_the_caller_lacks_is_never_added`); the wall re-checks every call (the
  raw-read resolution finding + `menu is a hint, wall is the law`).
- ✅ **The grounding test (#2):** a persona-grounded run is fed its pinned `core.e2e-backend` runbook
  BODY (`make dev` reaches the model's context) with a focused menu and NO repo/fs tool —
  `agent_persona_test.rs::a_persona_grounded_run_is_fed_its_pinned_skill_body_not_the_repo`.
- ✅ **The confusion fix, demonstrated (#3):** the same task, same caller — the reachable palette
  narrows 11→1 under `builtin.data-analyst` (off-task palette tools gone) —
  `agent_persona_catalog_test.rs::the_confusion_fix_...`. (Recorded finding: the menu is the palette
  catalog, not the full verb surface; on a bare node identity+grounding carry most of the cure.)
- ✅ **The coding posture (#4):** the extension-builder persona reaches the REAL devkit (a real
  `devkit.scaffold` lands an extension tree), and a real run proposing `ext.publish` **durably
  suspends** on the Ask floor (never on its own); a member caller is denied the devkit surface at the
  wall; an external runtime pairing fails at run start —
  `agent_persona_coding_test.rs` (10 tests). The full build+publish chain rides the existing
  `devkit_e2e_test.rs`/`ext_publish_test.rs`.

**Tests:** 21 (#1 model) + 8 (#3 catalog) + 10 (#4 coding) host tests + 6 UI gateway tests (#1
Settings) + the #2 seed/grounding tests — all green; existing agent + core-skills suites no-regression;
`node --features external-agent` builds.

## Related

- `scope/agent/agent-close-out-scope.md` — the finish-line sibling; its "curated tool menu"
  deferral resolves **here** (non-goal note updated to point at this topic).
- `scope/external-agent/external-agent-scope.md` — the profile thesis this productizes;
  `acp-driver-scope.md` (the scoped-not-built `granted_tools`/`persona_skill` fields #1 absorbs);
  `capability-wall-scope.md` (#4's sandbox dependency).
- `scope/skills/core-skills-scope.md` + `public/skills/skills.md` (the catalog/grant/inject
  substrate), `scope/agent-run/agent-run-scope.md` Part 5 (catalog inject + `skill.activate`),
  `scope/agent-memory/agent-memory-scope.md` (the no-partition decision kept).
- `scope/mcp/mcp-scope.md` (`tools.catalog`, the chokepoint), `scope/nav/`, `scope/channels/`,
  `scope/genui/`, `scope/frontend/dashboard/render-template-widget.md`, `scope/flows/` — the
  surfaces the built-in personas curate (#3).
- `docs/testing/` — the runbook corpus #2 promotes. README `§6.5`, `§6.16`, `§7`.
