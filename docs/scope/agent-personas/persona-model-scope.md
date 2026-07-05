# Agent-personas scope — the persona record & run assembly (persona-model)

Status: scope (the ask). Sub-scope #1 of `agent-personas-scope.md` — the foundation.
Promotes to `public/agent-personas/`.

Define the **persona record**, its CRUD + selection verbs, and the one place it is **applied**:
run assembly on the shared dispatch seam, for both runtimes. After this slice, a hand-authored
persona already fixes the observed confusion; the built-in catalog (#3) is then pure data.

## Goals

- **The record.** A workspace-facing `persona:{id}` bundle:

  ```
  Persona {
    id,                    // builtin.<slug> (reserved ns, read-only) | ws-scoped slug
    label, description,    // picker-facing
    identity: String,      // short persona prompt, prepended to SYSTEM_PROMPT / folded into goal
    granted_tools: Vec<String>,   // tool ids or trailing-* globs ("flows.*") — OPAQUE data (rule 10)
    grounding_skills: Vec<String>,// skill ids pinned at session start (grant-gated, fail-closed)
    extends: Vec<String>,  // persona ids whose tool/skill lists union in (identity: child wins)
  }
  ```

  Two tiers, one shape — the `agent_definition`/core-skills pattern, **fourth reuse**: built-ins
  seeded boot-idempotent from an embedded `personas.toml` into a reserved `_lb_personas`
  namespace (read-only; `builtin.*` writes rejected `BadInput` before the caps gate); custom
  personas are workspace-scoped records with admin CRUD.
- **Verbs** (one per file, `crates/host/src/agent/personas/`): `agent.persona.list` /
  `agent.persona.get` (member — the picker read) and `agent.persona.create` / `update` / `delete`
  (admin, custom-only) — caps `mcp:agent.persona.<verb>:call`. API shape: CRUD + get/list; no
  live feed (personas change rarely; the picker refetches), no batch.
- **Selection.** `agent.config` gains one additive optional `active_persona` (the
  `active_definition` move exactly — same record, same MERGE-patch, same
  invalidation); `agent.invoke` gains an optional `persona` argument as the per-run override
  (explicit > active > none). A stored persona that no longer resolves → **registry-default
  behavior + `warn!`**, never an errored run (the shipped `resolve_effective_runtime` posture).
  Personas are **orthogonal to definitions**: the same data-analyst persona runs on the in-house
  runtime or Open Interpreter — persona picks *focus*, definition picks *(runtime, model)*.
- **Application — the whole point.** At run assembly (the ONE seam both doors share,
  `invoke_via_runtime` → `dispatch.rs` / `run.rs`), a resolved persona:
  1. **Narrows the menu**: `RunContext.tools` = `reachable_tools(…) ∩ persona.granted_tools`
     (glob = opaque prefix match). For the **external** runtime the MCP bridge advertises only
     this narrowed set — protocol-level narrowing, the acp-driver `granted_tools` field finally
     built, absorbed here.
  2. **Sets identity**: in-house — `identity` prepended to `SYSTEM_PROMPT`; external — folded
     at the head of the goal (before catalog/memory, the shipped substrate-fold slot).
  3. **Pins grounding**: `grounding_skills` bodies are loaded via the shipped grant-gated
     `load_substrate_skill` path and injected at session start; `render_catalog` is filtered to
     the pinned set (the advertised skills list matches the persona's focus).
- **Narrowing, never widening — enforced, not asserted.** The wall is untouched: every dispatch
  still re-runs `caps::check` under the derived `agent ∩ caller` principal. Persona lists are
  hints to the *advertised* surface only. A persona naming a tool the caller lacks: still denied.
  A granted tool absent from the persona: out of the menu, but a model that proposes it anyway
  hits the unchanged wall (and for the external runtime, isn't advertised at all).

## The Settings surface — three dials, one boundary

Settings → Agent grows from one pane (the shipped definition catalog) to **three**, identical
for the internal and external runtime (both ride the same seam — no runtime branch in the UI):

1. **Agent — who runs.** The shipped definition catalog: runtime (in-house / external profile)
   × model + sealed key. Unchanged.
2. **Persona — what for.** The picker (built-ins + custom) and the custom-persona editor: a
   per-area **verb checklist** (grouped the way `persona-catalog` groups them, tool ids as
   opaque data), the grounding-skill list with per-skill **grant status** + the admin
   grant-batch affordance (#2), and the identity text. Member-visible, admin-editable
   (the `agent.persona.*` caps).
3. **Permissions — how supervised.** The per-tool **Allow / Ask / Deny** policy editor over the
   *shipped* `agent.policy.set` machinery (backend exists since agent-run Part 2; this is its
   first Settings surface). Shows the persona's `policy_preset` (#4) as the floor; tightening
   is free, loosening below the preset is the explicit admin write.

**The boundary (load-bearing): Settings edits advertisement + supervision, never the wall.**
No pane grants or revokes a capability. Instead, an **"Effective tools"** read-only view shows
the live result — `persona ∩ agent ∩ caller` for the viewing admin, computed from the real
`tools.catalog` — with each exclusion labelled by *why* (not in persona / not granted /
policy-denied) and a link out to the Roles & Grants surface for the caps themselves. Rejected:
inline grant-editing here — it would couple entitlement to focus (the exact coupling the
"personas as grants" rejection above forbids) and put an admin security surface behind an
agent-tuning page.

Tests (UI gateway, real spawned node): the effective view matches a real run's menu for the
same principal; the policy pane round-trips `agent.policy.set` and a run under the edited
policy actually suspends on an Ask'd tool; a member sees pickers read-only and no policy pane.

## Non-goals

- The built-in persona contents (verb lists, skill sets) — #3 (`persona-catalog-scope.md`).
- Creating the grounding skills themselves — #2 (`persona-grounding-scope.md`).
- The extension-builder persona's supervision posture — #4 (`persona-coding-scope.md`).
- **Persona-partitioned agent memory** — deliberately NOT added; memory stays workspace +
  member scoped (the agent-memory decided posture; a persona switch must not amnesia the
  workspace). Revisit only on a real contamination symptom.
- Per-persona model/budget policy — model rides the definition; budget rides
  `agent-close-out` B.

## Intent / approach

**Reuse the three shipped patterns wholesale; add one record and one intersection.**
The record/tiers/seed = the `agent_definition` + core-skills pattern (third and fourth reuse —
proven CRUD, reserved-ns, idempotent-seed code paths to crib). Selection = the
`active_definition` pointer pattern on `agent.config`. Application = three already-shipped seams
(`reachable_tools`, `SYSTEM_PROMPT`/goal-fold, `render_catalog` + `load_substrate_skill`) each
gaining one persona-aware filter/prepend. No new plumbing.

**Rejected: persona fields ON `agent_definition`.** A definition binds *(runtime, model)*; a
persona binds *focus*. Merging them multiplies the catalog (every persona × every model) and
breaks "same persona, either runtime". Separate record, two pointers on `agent.config`.

**Rejected: narrowing via `roles.define` cap bundles** (the only narrowing possible today).
Caps are admin-owned entitlements; personas are freely-switched focus. Also cap-narrowing can't
express skill pinning or identity, and would make persona switching a grants churn.

**Rejected: hard-enforcing `granted_tools` as a second wall** (denying at dispatch what the
persona omits). Two walls drift; the menu-is-a-hint / wall-is-the-law split is the shipped,
tested posture (`default-agent-wiring`) — keep it. The external protocol-level narrowing is
advertisement, not authorization.

## How it fits the core

- **Tenancy / isolation:** custom `persona:{id}` is ws-scoped; built-ins live in `_lb_personas`
  (readable everywhere, writable nowhere); `active_persona` rides the ws-scoped `agent.config`;
  a ws-B run can never resolve ws-A's persona (mandatory isolation test).
- **Capabilities:** new caps `mcp:agent.persona.{list,get}:call` (member) +
  `{create,update,delete}` (admin). Deny paths: non-admin write denied; `builtin.*` write
  `BadInput`; a pinned-but-ungranted skill **fails the run at session start** (fail-closed, the
  acp-driver decision); persona-listed-but-uncapped tool denied at the unchanged wall.
- **Placement:** either. Seed + resolution are node-symmetric; no role branch.
- **MCP surface:** the five `agent.persona.*` verbs + two additive optional fields
  (`agent.config.active_persona`, `agent.invoke.persona`). CRUD + get/list only (shape argued
  above). Tool ids inside `granted_tools` are **opaque strings** — no host code branches on
  them (rule 10; an extension tool `mqtt.publish` curates identically to a host verb).
- **Data (SurrealDB):** one new table (`persona`, SCHEMAFULL, the `agent_definition` shape
  discipline) + one nullable field on `agent.config`. State only.
- **Bus:** none.
- **Secrets:** none — personas are names-only by construction (no endpoint, no key).
- **Stateless / hot-reload:** persona is read at run assembly; nothing held in an instance.
- **SDK/WIT impact:** none.
- **No mocks (rule 9):** tests seed real persona records via the real verbs and run the real
  loop/gateway; external-runtime tests ride the existing real-subprocess smoke seam.
- **File layout:** `agent/personas/{model,seed,list,get,create,update,delete,resolve,apply}.rs`
  — one verb/responsibility per file; `apply.rs` is the single run-assembly filter both
  `run.rs` and `dispatch.rs` call.
- **Skill doc:** `skills/agent/SKILL.md` gains a "Personas" section (pick, override, author)
  in this slice's session, grounded in a live run.

## Example flow

1. Admin creates `persona:data-analyst-custom` via `agent.persona.create` (or picks the #3
   built-in) and sets `agent.config { active_persona: "builtin.data-analyst" }` in Settings →
   Agent.
2. A member posts to their dock. `agent.invoke` (no explicit persona) resolves active persona +
   active definition; `apply.rs` narrows the menu to the persona's tools ∩ caller grants, loads
   the pinned skills (grant-checked — one missing grant would fail the run here, with a named
   error), prepends the identity.
3. The external runtime launches; its MCP bridge advertises **only** the narrowed tool set; the
   goal opens with the persona identity + pinned skill bodies + filtered catalog + memory index.
4. The agent works focused. It proposes `flows.save` (not in the data-analyst persona) — not
   advertised; had it been proposed anyway (in-house), the wall re-check governs as today.
5. The member overrides per-message once (`persona: "builtin.flow-author"`) — that run only.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), real store/bus/caps/gateway/loop:

- **Capability-deny (§2.1):** non-admin `agent.persona.create` denied, nothing persists;
  `builtin.*` update rejected; persona-listed tool the caller lacks → dispatch denied (no
  widening — THE headline); pinned ungranted skill → run fails at start with the named error,
  no model call spent.
- **Workspace-isolation (§2.2):** ws-B cannot `get`/apply ws-A's custom persona; ws-B's
  `active_persona` never affects a ws-A run; built-ins readable from both, writable from
  neither.
- **Offline/sync (§2.3):** persona LWW on concurrent update (the `agent.config` posture);
  deleted-while-active persona → warn + registry-default behavior on next run, no errored run.
- **Both-runtimes application:** in-house — menu narrowed, identity in system prompt, catalog
  filtered (assert the exact injected messages); external — bridge advertises only the narrowed
  set (assert the ACP-advertised tool list), goal carries identity + pinned bodies. The swap
  test: a record-only persona changes all three with zero code change.
- **Resolution precedence:** explicit invoke `persona` > `active_persona` > none; unknown
  explicit id → named error (an explicit ask must not silently degrade).
- **Units:** glob matching (trailing-`*` only; `*` alone rejected on write — an
  everything-persona is "no persona", say so); `extends` union + cycle rejection at write time;
  idempotent re-seed.

## Risks & hard problems

- **Two pointers on `agent.config`** (`active_definition`, `active_persona`) invite UX
  confusion. The Settings surface must present ONE mental model: *agent = who runs (definition)
  × what for (persona)*. Copy matters; test the picker flow.
- **Catalog filtering vs. discoverability.** Filtering `render_catalog` to pinned skills means
  the model can't see other granted skills exist. That's the point (focus), but `skill.activate`
  of an unpinned-but-granted skill stays allowed (wall = grant, menu = hint) — document it so
  the behavior isn't read as a bug.
- **Glob semantics rot.** `flows.*` must mean prefix-on-the-tool-id and nothing smarter — no
  cap-grammar interplay. One function, one file, property-tested.
- **`extends` depth.** Keep it one level of union at *write* time (materialize the closure into
  the stored record? No — resolve at read, cycle-checked at write, depth-capped at 3) — decide
  in implementation, but the cap must exist or a cycle/deep chain becomes a boot hazard.

## Open questions

1. **Materialize `extends` at write vs resolve at read?** Proposal: resolve at read (parents
   evolve → children follow, matching the rules-author ask "auto-load flows + data"), cycle-check
   + depth ≤ 3 at write.
2. **Does an active persona apply to `agent.def.test`?** Proposal: yes — the test button should
   prove the context the run will actually get (its `context:` line gains `persona`).
3. **Per-channel/per-surface default personas** (dock ≠ Data Studio)? Proposal: defer; one
   workspace default + per-invoke override first; the surface can pass the override itself
   (Data Studio invoking with `persona:"builtin.widget-builder"` needs no new mechanism).
4. **Empty `granted_tools`** = tool-less conversational persona — allowed? Proposal: yes,
   explicitly (useful for a pure-Q&A grounded persona); `[]` ≠ unset (unset = no narrowing).

## Related

- `agent-personas-scope.md` (umbrella), `persona-grounding-scope.md` (#2),
  `persona-catalog-scope.md` (#3), `persona-coding-scope.md` (#4).
- Shipped substrate this rides: `scope/agent/agent-catalog-scope.md` (record/tier/seed pattern +
  `active_definition` pointer), `scope/skills/core-skills-scope.md` (grant gate + catalog
  inject), `scope/agent-run/agent-run-scope.md` Part 5 (`skill.activate`),
  `scope/external-agent/acp-driver-scope.md` (the `granted_tools`/`persona_skill` fields this
  absorbs and builds).
- `rust/crates/host/src/agent/{menu.rs,catalog.rs,run.rs,dispatch.rs}` — the four seams
  `apply.rs` touches; `rust/crates/host/src/tools/catalog.rs` (`tools.catalog`).
- README `§6.16`, `§6.5`, `§7`.
