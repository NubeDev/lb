# Persona-coding (agent-personas #4) — session log

Status: **SHIPPED**. Scope: [`scope/agent-personas/persona-coding-scope.md`](../../scope/agent-personas/persona-coding-scope.md)
(umbrella: [`agent-personas-scope.md`](../../scope/agent-personas/agent-personas-scope.md)).
Depends on #1–#3. The persona with a safety posture of its own.

## The ask, restated

`builtin.extension-builder` — "100% coding, but never on its own." The agent builds UI/WASM/native
EXTENSIONS against the devkit, in a scoped workdir, supervised via the shipped Ask policy. The one #1
code addition #4 needs: the persona record's `policy_preset` + `runtimes` fields, applied at run
assembly (a supervision floor + a runtime restriction).

## What shipped

**The persona** (`personas.toml`, the 8th entry): the devkit surface (`devkit.*`, `host.fs.list`,
`ext.*`, `native.*`) + the verify loop (`tools.catalog`, `bus.watch`, `telemetry.read`, `system.tools`)
— all admin-tier, so only useful to an admin caller; pins `core.extension-authoring` (#2, the devkit
manual), `core.extensions`, `core.e2e-backend`; carries the safety posture:
- `policy_preset.ask = [ext.publish, ext.uninstall, ext.disable, native.install, native.reset]` — the
  node-mutating verbs. The edit/build inner loop (`devkit.scaffold/build/inspect`) stays Allow (fluid).
- `runtimes = ["default"]` — in-house-only until the external-agent capability-wall sandbox ships.

**The one code addition** — the `policy_preset` FLOOR, applied at run assembly:
- `clamp_to_preset(ws_effect, tool, ws_policy, preset)` in `personas/apply.rs` — threaded via
  `RunContext.persona_preset` → `run.rs`, applied per proposed call **after** `evaluate(ws_policy)`.
- `check_runtime` (built in #1) enforces the `runtimes` restriction — a non-`default` pairing fails at
  run start with the named `AgentError::PersonaRuntime`, before any subprocess.

## The load-bearing design finding (a clamp, not a merged rule list)

My first `policy_preset` implementation **merged** the preset into the ws policy's rule list (append an
Ask rule per gated tool). **A failing test caught that this is broken:** the shared evaluator's
precedence is **Deny > Allow > Ask** (an Ask is the *weakest* — "if any rule already Allows, there's
nothing to ask"). So an appended Ask rule can NEVER beat a blanket `*`-Allow — the supervision floor
would silently evaporate under a workspace that Allows everything.

**Fix — the floor is a CLAMP, not a rule:** `clamp_to_preset` runs *after* `evaluate` and raises the
evaluated effect for a preset tool (Allow → Ask; preset Deny → Deny), **unless** the ws policy has an
**explicit** (exact-tool, no-glob) rule for it — that explicit rule IS the auditable "loosening below
the preset is the admin's explicit write" the scope requires. A blanket `*` rule is NOT explicit, so it
can't silently loosen the floor. The evaluator's own precedence stays untouched (correct for an admin
policy); the floor is a thin, pure, order-independent layer above it. Proven by
`a_blanket_ws_allow_does_not_loosen_the_floor_but_an_explicit_rule_does`.

## A real bug caught by a test (TOML sub-table binding)

`the_extension_builder_is_readable...` failed: the persona's `runtimes` came back `None` despite
`runtimes = ["default"]` in the manifest. Root cause: in TOML, once a `[persona.policy_preset]`
sub-table header opens, subsequent bare keys bind to **it**, not the parent — so `runtimes` (authored
*after* the sub-table) was parsed as `policy_preset.runtimes` and dropped. **Fix:** author `runtimes`
BEFORE the `[persona.policy_preset]` sub-table (a comment in the manifest flags the trap). This is
exactly the kind of silent mis-seed the tests exist to catch.

## Tests — `crates/host/tests/agent_persona_coding_test.rs` (10 green)

Real Node/store/caps/loop; MockProvider the only stub.

- `a_member_caller_under_the_persona_is_denied_devkit_and_publish` — **caps-deny (§2.1)**: a member
  driving `devkit.scaffold`/`ext.publish` through the full `call_tool` dispatch is denied at the wall
  (admin-tier; the persona advertises, the wall withholds) — nothing scaffolds, nothing publishes.
- `the_persona_carries_the_ask_preset_on_node_mutating_verbs` — the preset gates the 5 node-mutating
  verbs (Ask) and leaves the inner loop Allow.
- `preset_floors_an_empty_ws_policy_to_ask` + `a_blanket_ws_allow_does_not_loosen_the_floor_but_an_explicit_rule_does`
  — the FLOOR: Ask over an empty/blanket-Allow policy; loosened only by an explicit per-tool rule; a
  preset Deny is absolute.
- `activating_the_persona_with_an_external_runtime_fails_with_a_named_error` +
  `the_persona_runs_fine_on_the_in_house_default_runtime` — the **runtime restriction** (named
  `PersonaRuntime` error before any subprocess; in-house runs fine).
- `the_extension_builder_is_readable_in_every_workspace_but_seeded_once` — **ws-isolation** (built-in
  union; `runtimes` round-trips).
- `a_hostile_scaffold_id_is_rejected_not_a_filesystem_escape` — **adversarial devkit hardening**: path
  traversal / non-kebab / reserved-prefix ids are cleanly rejected (typed error), never a traversal or
  a panic — the devkit's `validate_id` is the boundary an agent will fuzz.
- **E2E:** `a_real_scaffold_works_through_the_persona_devkit_surface` (a REAL `devkit.scaffold` lands a
  real extension tree — manifest + build.sh — through the persona's surface) +
  `a_publish_proposed_under_the_persona_suspends_for_a_human_it_never_publishes_on_its_own` (a **real
  run** under the persona where the model proposes `ext.publish` **durably SUSPENDS** on the Ask floor —
  `JobStatus::Suspended` + a `SuspensionOpened` awaiting a human `agent.decide`). The **"never on its
  own"** guarantee, proven end to end.

```
$ cargo test -p lb-host --test agent_persona_coding_test
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The full scaffold→build→**publish with a real cargo build**→call-the-new-tool chain is already proven
in `devkit_e2e_test.rs` + `ext_publish_test.rs` — #4 does not duplicate the heavy build; it proves the
persona-driven surface + the supervision gate on top of it.

## Open-question resolutions (scope §Open questions)

1. **Publish-approval reviewer** → any workspace admin (matches `workflow.resolve_approval`); the
   suspension is workspace-owned (the durable `agent_decision`, settled by `agent.decide` under the
   member/admin cap). No per-invoker binding added.
2. **`devkit.build` output streams or blob?** → blob first (the verb's shipped shape); streaming is the
   `agent-close-out` C follow-up. Unchanged by #4.
3. **UI-extension preview** → scratch-workspace publish v1 (the persona proposes `ext.publish` into the
   workspace registry; the preview seam is Studio's scope). Unchanged.

## Front line / next

The topic is complete: **all four sub-scopes shipped**. The umbrella exit gate is met (see the umbrella
scope's gate + STATUS). This session's #4 is the last of the five gate bullets (the coding posture).
