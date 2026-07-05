# Persona-model (agent-personas #1) — session log

Status: in-progress. Scope: [`scope/agent-personas/persona-model-scope.md`](../../scope/agent-personas/persona-model-scope.md)
(umbrella: [`agent-personas-scope.md`](../../scope/agent-personas/agent-personas-scope.md)).
Stage: post-S8, building on the shipped agent + skills + catalog substrate.

## The ask, restated

A run today gets everything — the constant `"You are a workspace agent."` prompt, the caller's whole
reachable tool catalog, no task grounding — and the (external) agent is confused. Build the **persona
record** + its CRUD/selection verbs + the ONE run-assembly application (menu narrow, identity, skill
pin) for BOTH runtimes, as *data, narrowing, never widening*. Exit gate: the swap test + the narrowing
test, both runtimes.

## What was built (front line)

Backend, all under `rust/crates/host/src/agent/personas/` (one verb per file, the `defs/` pattern —
fourth reuse of the record/tier/seed pattern):

| File | Responsibility |
|---|---|
| `model.rs` | `Persona { id, label, description, identity, granted_tools, grounding_skills, extends, policy_preset?, runtimes?, builtin }` + `PolicyPreset` + `builtin.` reserved rule. |
| `store.rs` | schemaless `persona` table; raw get/list/upsert/delete over a namespace; `PERSONA_NS = _lb_personas`. |
| `validate.rs` | reserved-tier reject (before caps), glob grammar (trailing-`*` only, bare `*` rejected), `extends` self/dup + cross-store cycle/depth (≤3) walk. |
| `seed.rs` | boot seeder from embedded `personas.toml` (override `LB_PERSONA_CATALOG_TOML`); only writer of `_lb_personas`. |
| `list/get/create/update/delete.rs` | the five gated verbs (`mcp:agent.persona.<verb>:call`; list/get member, C/U/D admin). |
| `tool.rs` | MCP bridge `call_agent_persona_tool` (`agent.persona.*` → `{personas}`/`{persona}`/`{ok}`). |
| `resolve.rs` | `resolve_persona` (explicit invoke arg > `active_persona` > none; explicit-unknown = named error, dangling active = warn + un-narrowed) + `resolve_effective` (the `extends`-closure union, child-wins identity). |
| `apply.rs` | the ONE run-assembly filter both runtimes call: `narrow_tools` (glob-aware ∩), `build_identity_fold` (identity + pinned-skill BODIES into goal, **fail-closed** on ungranted skill), `check_runtime` (the #4 runtime restriction). |

Selection: `AgentConfig` gained `active_persona: Option<String>` (+ schema field + column) — the
`active_definition` move exactly. `agent.invoke` gained a per-invoke `persona` override threaded through
every front door: `AgentPayload` → `ChannelAgentJob` → `agent_worker::drive_run` → `invoke_via_runtime`
(top-level `persona` param, mirroring `runtime`); the routed `AgentInvokeRequest` + `invoke_remote` +
`serve`; the gateway `POST /agent/invoke` `InvokeRequest.persona`.

Application (the point), in `invoke_via_runtime` (dispatch.rs — the ONE seam both doors share):
1. resolve persona → `EffectivePersona` (extends unioned);
2. `check_runtime` (fail-closed, before any model spend);
3. narrow `tools` to `persona ∩ reachable` (glob prefix) → the `RunContext.tools` both the in-house
   model menu AND the external bridge advertise;
4. fold identity + pinned-skill bodies into `goal` (reaches both runtimes — goal seeds the in-house
   rehydrate and is the external agent's only channel), fail-closed on an ungranted pin;
5. filter the advertised catalog to the pinned set (`render_catalog_filtered`) — external in dispatch.rs,
   in-house via a new `RunContext.persona_catalog` field consumed in `run.rs`.

Caps added to the dev-login member set (`credentials.rs`): `agent.persona.{list,get}` (member),
`{create,update,delete}` (admin). Boot seed wired into `node/src/main.rs` + `test_gateway.rs`.

## Key decisions (the why)

- **`policy_preset` + `runtimes` fields added to the record NOW**, even though #1 barely uses them, because
  #4 (persona-coding) requires them and adding a stored field later would churn the schema + every seed.
  They are nullable + `skip_serializing_if` so a #1-era record is byte-identical without them. This is
  the "build the whole contract" rule applied to the record shape.
- **Persona resolution is its OWN resolver, not a field on `resolve_active_definition`.** A persona is
  *focus*; a definition is *(runtime, model)*. Same shape, one concept over — orthogonal, so the same
  persona rides either runtime (the scope's "same data-analyst on in-house or Open Interpreter").
- **The external ACP runtime's tool-advertisement bridge is NOT in this repo tree** (it's the
  feature-gated role crate, an unbuilt slice). So "both runtimes" is proven honestly: narrowing
  `RunContext.tools` upstream is exactly what the real bridge advertises, and a **scripted external
  runtime** in the test captures that `RunContext` to assert the narrowed set + folded identity. This is
  the same seam every real runtime uses (`agent_runtime_seam_test` / the role crate's `swap_test.rs`
  precedent), not a mock of the bridge.
- **Identity + pinned bodies bake into the `goal`, not a separate system message**, so ONE code path
  covers both runtimes (the goal seeds the in-house rehydrate and is the external agent's only input).
  The catalog *filter* (name+description) is the one thing that differs by call site, handled by the new
  `render_catalog_filtered(pinned)` at both sites.
- **Fail-closed on an ungranted pinned skill** = a named `AgentError::PersonaSkill` at run start, before
  any model call — the acp-driver decision, kept. Not opaque `Denied`: the caller *chose* this persona
  and must see why it won't run.
- **`ToolError → AgentError` mapping** (`tool_to_agent` in dispatch.rs) preserves the named
  BadInput/NotFound for an explicit-unknown persona (an explicit ask must not silently degrade), while a
  deny stays opaque.

## Open-question resolutions (scope §Open questions)

1. **Materialize `extends` at write vs read?** → **resolve at read** (`resolve_effective`), cycle-checked
   + depth-capped (≤3) at write (`validate_extends`). Parents evolve → children follow (the rules-author
   ask). Done.
2. **Does an active persona apply to `agent.def.test`?** → **no, for now** — `agent.def.test` is a
   runtime/model diagnostic (a self-describe turn); previewing the persona's context is a separate
   follow-up. Recorded at the `defs/test.rs` call site. (Deviates from the scope's "proposal: yes" — the
   honest v1 keeps the diagnostic isolated; revisit when the Settings "test with persona" affordance lands.)
3. **Per-surface default personas?** → deferred; the per-invoke `persona` arg already lets a surface
   (Data Studio → `builtin.widget-builder`) override with no new mechanism (`InvokeRequest.persona` is
   wired). Done as scoped.
4. **Empty `granted_tools` = tool-less conversational persona?** → **yes, explicitly.** `[]` → empty menu
   (distinct from *unset* = no narrowing, which never reaches `narrow_tools`). Enforced in `apply.rs`.

## Tests

`crates/host/tests/agent_persona_test.rs` — real Node/store/caps/loop, MockProvider the only stub:
CRUD roundtrip; non-admin create denied + nothing persists; `builtin.*` write rejected before caps;
idempotent re-seed + built-ins readable-everywhere/writable-nowhere; ws-B can't get ws-A custom persona;
**swap test in-house** (record-only persona narrows menu + folds identity, a recording model captures
both); **swap test external** (scripted external runtime advertises the narrowed set + folded identity);
**narrowing** (a persona tool the caller lacks is never added); **fail-closed** (ungranted pin →
`PersonaSkill` error, no model spend); resolution precedence (explicit > active; explicit-unknown =
error; dangling active = un-narrowed); `extends` union + self/two-node cycle rejection; glob unit.

Plus the Settings read verbs: `agent.persona.resolve` (extends-unioned effective persona for the
effective-tools view; deny-tested) and `agent.policy.get` (round-trips `agent.policy.set`; deny-tested).

```
$ cargo test -p lb-host --test agent_persona_test
running 19 tests
test glob_matches_prefix_and_literal ... ok
test a_pinned_ungranted_skill_fails_the_run_at_start_with_a_named_error ... ok
test swap_test_external_runtime_advertises_narrowed_tools_and_folds_identity ... ok
test non_admin_create_is_denied_and_nothing_persists ... ok
test a_two_node_extends_cycle_is_rejected_at_write ... ok
test builtin_write_is_rejected_before_the_caps_gate ... ok
test explicit_persona_overrides_the_active_one ... ok
test narrowing_a_persona_tool_the_caller_lacks_is_never_added ... ok
test swap_test_in_house_menu_and_identity_reflect_a_record_only_persona ... ok
test an_explicit_unknown_persona_is_a_named_error_not_a_silent_run ... ok
test crud_roundtrips_for_an_admin ... ok
test a_dangling_active_persona_warns_and_runs_un_narrowed ... ok
test extends_unions_parent_tools_and_skips_a_self_cycle ... ok
test builtins_seed_readable_everywhere_writable_nowhere_idempotent ... ok
test ws_b_cannot_get_ws_a_custom_persona ... ok
test resolve_verb_returns_the_extends_unioned_effective_persona ... ok
test resolve_verb_is_denied_without_the_cap ... ok
test policy_get_round_trips_the_set_policy ... ok
test policy_get_is_denied_without_the_cap ... ok
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Plus: the existing agent suite (`agent_runtime_seam`, `agent_in_house_wiring`, `agent_default_runtime`,
`agent_config`, `channel_agent_worker`, `agent_page_context`) stays green (no regression from the new
`persona`/`persona_catalog` params); `lb-role-external-agent` tests green (the real external runtime's
`RunContext` literals updated); `node --features external-agent` builds; `cargo fmt --check` clean.

## Correctness finding during the session (worth recording)

**Run-assembly persona resolution must NOT require the caller to hold `mcp:agent.persona.get`.** The
first test run failed 4/19: a member with `agent.invoke` but not the persona *picker* read cap got its
active persona silently dropped (or an explicit persona denied). Root cause: `resolve_persona` was
calling the cap-gated `agent_persona_get` verb. **Fix:** run-assembly reads the persona via a raw,
namespace-walled store read (`read_persona_for_assembly` in `resolve.rs`), NOT the picker cap. Rationale:
a persona read at run assembly can only ever *narrow* (remove tools, pin skills) — it never widens a
capability, so gating it on the picker cap guards nothing while breaking the common case. The workspace
wall still holds (namespace-scoped: a ws-B run can't read a ws-A custom persona; built-ins from the
reserved union). The CRUD *verbs* keep their cap gate for the Settings UI. This is the persona analog of
"the menu is a hint, the wall is the law": persona resolution is advertisement-shaping, not authorization.
No debug entry (caught by the test before merge, pre-integration), but recorded here as the load-bearing
design call.

## Front line / next

- Getting the full workspace + `--features external-agent` green (existing call sites gained the new
  `persona`/`persona_catalog` args).
- Settings surface (persona pane, Allow/Ask/Deny policy pane over the shipped `agent.policy.set`,
  read-only effective-tools view) + gateway wiring + Vitest.
- `skills/agent/SKILL.md` persona section (grounded in a live run).
- Then #2 (grounding), #3 (catalog), #4 (coding).

## UI — the Settings surface (three dials, one boundary)

Built the persona Settings surface under `ui/`, cloning the shipped agent-definition catalog
patterns. All over the generic `/mcp/call` bridge — zero gateway changes.

| File | Responsibility |
|---|---|
| `ui/src/lib/agent/agentPersona.api.ts` | the five persona verbs + `resolveEffectivePersona(id?)`; `Persona`/`PersonaPatch`/`EffectivePersona`/`PolicyPreset` types (mirror the Rust structs). |
| `ui/src/lib/agent/policy.api.ts` | `getAgentPolicy()`/`setAgentPolicy(rules)` + `Rule`/`Effect`/`ArgMatch` types. |
| `ui/src/lib/agent/config.api.ts` | added the additive optional `active_persona` field to `AgentConfig`. |
| `ui/src/lib/session/admin-caps.ts` | added `agentPersona{List,Get,Resolve,Create,Update,Delete}` + `agentPolicy{Get,Set}` CAP constants. |
| `ui/src/features/settings/agent/usePersonaCatalog.ts` | data hook: list + active-pick via `agent.config.set{active_persona}` + create/update/remove + reload (clone of `useAgentCatalog`). |
| `ui/src/features/settings/agent/PersonaCatalog.tsx` | list/picker with Built-in/Custom/Active badges, cap-gated Use/Edit/Delete (Edit/Delete also `!builtin`). |
| `ui/src/features/settings/agent/PersonaEditor.tsx` | create/edit form: label, identity textarea, granted_tools + grounding_skills list editors, `extends` multiselect. |
| `ui/src/features/settings/agent/StringListField.tsx` | the add/remove `string[]` list primitive the editor reuses. |
| `ui/src/features/settings/agent/EffectiveTools.tsx` | read-only `persona ∩ agent ∩ caller`: resolves the persona + reads `tools.catalog`, marks each granted_tools entry included/excluded with a reason; shows identity + pinned skills; links out to Roles & Grants (NO grant editing). |
| `ui/src/features/settings/agent/PolicyPane.tsx` | the Allow/Ask/Deny editor over `agent.policy.get`/`set`; shows a persona `policy_preset` as the FLOOR (marks preset rows, warns on loosening — v1 visual-only, not hard-blocked). |
| `ui/src/features/settings/agent/PersonaSection.tsx` | composes the three panes; wired into `AgentTab.tsx` below the definition catalog. |

**The boundary held:** no pane grants/revokes a capability. The editor + picker tune advertisement
(the narrowed menu + identity + pinned skills); the policy pane tunes supervision (Allow/Ask/Deny);
EffectiveTools shows the live wall result read-only. Persona/tool ids are opaque strings throughout
(rule 10) — no branch on a specific id.

## UI test (real spawned gateway, no fakes — rule 9)

`ui/src/features/settings/PersonaSettings.gateway.test.tsx` — 6 tests, all green against the real
`test_gateway` (boot-seeds `builtin.data-analyst`):

1. picker lists the seeded built-in (Built-in badge, no edit/delete); picking writes `active_persona`
   (re-read `agent.config.get` asserts it).
2. admin create → edit → delete a custom persona (each a real verb; re-list confirms).
3. capability-deny: a member without the write caps sees no create/pick affordance and the policy
   pane read-only (no Save/Add rule).
4. capability-deny at the wall: `agent.persona.create` without the cap 403s, nothing persists.
5. EffectiveTools marks an out-of-catalog persona tool excluded (`not granted`).
6. PolicyPane round-trips an Ask rule via `agent.policy.set` (re-read `agent.policy.get` confirms).

Note: the shipped `Tabs` panel `Reveal` reads `useTheme`, so the gateway test wraps the harness in
`ThemeProvider` (the pre-existing `AgentCatalog.gateway.test.tsx` is currently red on master for
exactly this missing-provider reason — a separate pre-existing issue, not introduced here).
