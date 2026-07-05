# Persona-catalog (agent-personas #3) — session log

Status: **SHIPPED**. Scope: [`scope/agent-personas/persona-catalog-scope.md`](../../scope/agent-personas/persona-catalog-scope.md)
(umbrella: [`agent-personas-scope.md`](../../scope/agent-personas/agent-personas-scope.md)).
Depends on #1 (record + application) and #2 (the pinned skills). Zero new code — content only.

## The ask, restated

Ship the seven built-in personas as **data** (`personas.toml`): the exact tool allow-list + pinned
skills for data-analyst, flow-author, widget-builder, rules-author (extends flow+data), workspace-admin,
channels-operator, system-manager (extends all six). Prove per-persona narrowing, `extends` composition,
caps-deny, ws-isolation, and the **confusion before/after demo** (the umbrella gate).

## What shipped

`rust/crates/host/src/agent/personas/personas.toml` — the seven `[[persona]]` entries (replacing the #1
starter). Each carries the scope's exact `granted_tools` (globs = trailing-`*`) + `grounding_skills`
(≤ 4 body pins) + an identity that names the deny/hand-off posture. `rules-author` and `system-manager`
use `extends` (resolve-at-read union). The destructive-verb exclusion (`workspace.delete/purge`,
`authz.revoke-tokens`, `secret.get`) is enforced by omission from every persona.

**Cross-verification (I own this — a persona missing a verb is broken):** every non-glob `granted_tools`
id and every glob prefix was checked against the live verb inventory (`credentials.rs` `member_caps()`
∪ host-src verb registrations, 155 distinct verbs) — **zero missing**. Every `grounding_skills` id was
checked against the seeded corpus (34 skills) — **zero broken pins**. (Scripted check; output in the
session's scratch.)

## The load-bearing implementation finding (recorded in the scope)

**The reachable menu a persona narrows is the *palette-descriptor catalog + loaded extension tools*, NOT
the full ~175-verb surface.** `reachable_tools` reads `tools.catalog` = `host_descriptors()` ∩ caps, and
`host_descriptors()` (`crates/host/src/tools/descriptor.rs`) is a **curated palette** of ~11 host verbs
(`federation.query`, `query.*`, `agent.invoke`, `reminder.*`, `dashboard.catalog`/`pin`, secrets) plus
extension tools. Most host verbs (`rules.*`, `flows.*`, `dashboard.save`, `nav.*`, `roles.*`, …) are
**callable** but not palette-advertised, so a model never sees them in its menu — persona or not.

I **stopped and recorded this in `persona-catalog-scope.md` → "Implementation finding"** rather than
coding around it. Consequences (all honest):
- The persona `granted_tools` lists are the **complete forward-looking allow-list** — correct, just
  ahead of the palette. As verbs gain descriptors / arrive as extension tools, personas narrow them
  with zero change. NOT trimmed to only palette tools (that would rot the lists and lose the intent).
- The **narrowing mechanism** (`menu = reachable ∩ granted_tools`) is proven over what's genuinely
  reachable — a persona can only shrink the reachable set.
- The **confusion cure has two levers**; this clarifies which does the work *today*: on a bare node the
  tool-menu is already small, so the dominant cure is **identity + pinned grounding** (proven in #2's
  grounding test). Tool-narrowing bites hardest with many extension tools loaded (the real symptom).
- Follow-up (not a #3 blocker): widening `host_descriptors()` so more host verbs are palette-visible is
  its own scope — noted so the persona lists are known to be ready for it.

The tests assert narrowing over the **genuinely-reachable** palette tools (honest, non-drifting), and
prove `extends` composition at the **record level** via `agent.persona.resolve` (the full union,
including non-palette verbs the menu can't show).

## Tests — `crates/host/tests/agent_persona_catalog_test.rs` (8 green)

Real Node/store/caps/loop; the "admin caller" holds the **real dev-login cap set** via
`lb_role_gateway::dev_claims` (a dev-dependency — the honest cap source, not a hand-copied list that
drifts; a dev-dep cycle gateway↔host is permitted by Cargo).

- `all_seven_builtins_seed_with_tools_and_pins` — all 7 resolve, built-in, non-empty focus + identity.
- `each_persona_narrows_to_its_focus_for_an_admin_caller` — per persona: an in-focus **palette** tool
  present, an out-of-focus palette tool absent, menu ≤ full palette.
- `destructive_verbs_are_excluded_from_every_persona_even_for_an_admin` — `workspace.purge` etc. in NO
  persona's menu, even for the admin who holds the cap.
- `workspace_admin_persona_under_a_member_caller_advertises_nothing_it_lacks` — the caps-deny headline:
  `persona ∩ member`, the persona's admin verbs never reach a bare member (the wall withholds them).
- `rules_author_is_the_union_of_its_parents_plus_its_own` + `system_manager_composes_all_six_parents` —
  `extends` composition at the record level (`agent.persona.resolve` returns `rules.*` ∪ `flows.*` ∪
  `federation.query` …), and the palette subset reaches the run menu. Destructive verbs stay excluded
  even composed.
- `the_confusion_fix_the_same_task_narrows_from_the_whole_surface_to_the_focus` — **the umbrella gate**:
  same caller, same task; menu **11 palette tools → 1** under `data-analyst`; off-task palette tools
  (`reminder.create`, `dashboard.pin`) gone, on-task `federation.query` stays.
- `a_ws_b_active_persona_never_affects_a_ws_a_run` — ws-isolation: ws-B's active data-analyst narrows
  ws-B only; a ws-A run keeps its full palette (the pick rides the ws-scoped `agent.config`).

```
$ cargo test -p lb-host --test agent_persona_catalog_test
CONFUSION FIX: reachable palette 11 tools -> 1 tools under the data-analyst persona
              (identity + pinned grounding are the other half of the cure — see #2)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```
(No regression: #1's `agent_persona_test.rs` stays 21 green over the full manifest.)

## Open-question resolutions (scope §Open questions)

1. **`data-analyst` gets `ingest.write` by default?** → **yes** (listed) — member-tier + ws-walled.
2. **Picker UX for grant gaps** → activate + degrade with the honest run-start error (#1); the picker
   offers the grant batch. The UI (#1 Settings surface) surfaces per-skill grant status.
3. **Surfaces auto-override at invoke time** → deferred (as #1 Q3); the per-invoke `persona` arg already
   lets a surface pass its own focus.

## Front line / next

#4: `builtin.extension-builder` — seeds from this same `personas.toml` but carries the safety posture
(`policy_preset` Ask-gate + in-house-runtime restriction). The one remaining umbrella exit-gate bullet.
