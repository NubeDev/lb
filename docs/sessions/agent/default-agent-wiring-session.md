# Default agent — wiring the in-house brain + shared tool dispatch + boot (session)

- Date: 2026-07-03
- Scope: ../../scope/agent/default-agent-wiring-scope.md
- Stage: post-S8 (building on the shipped agent runtimes) — branch `master`
- Status: done

## Goal

Finish the always-registered in-house `"default"` agent so it actually works and can use the
platform's own tools. Four wiring seams (no new subsystems): (1) wire a real model into the in-house
runtime from node config; (2) route the loop's proposed tool calls through the ONE host MCP bridge
`call_tool` so the loop reaches host-native verbs (the load-bearing fix — it serves the external agent
too); (3) surface the caller's reachable `tools.catalog` to the loop; (4) boot it — build the registry
and call `serve_agent`, closing the serve-wiring TODO.

## What was available (state plainly, per the scope)

**No real AI-gateway provider adapter exists.** The only `Provider` in the tree is the sanctioned
test-only `MockProvider` (`role/ai-gateway/src/mock.rs`); the ai-gateway scope lists the real
OpenAI-compatible / local adapters as deferred. So the model-wiring deliverable here is the **real
wiring seam + config + the unconfigured→configured swap**, proven for real against
`AiGateway<MockProvider>` at the test boundary. Nothing is stubbed to *fake* "it answers": the boot
path builds the registry and installs whatever `ModelAccess` is configured; when a configured provider
has no adapter yet, boot keeps the honest `UnconfiguredModel` and logs the explicit swap point. The
moment a real `impl Provider` lands it drops into `build_in_house_model`'s match with no change
anywhere else.

## What changed

**The load-bearing fix — shared host-native tool dispatch** (`crates/host/src/agent/step.rs`):
`run_calls` now dispatches each proposed call through `crate::tool_call::call_tool` — the SAME entry
the gateway's `POST /mcp/call` uses — instead of `lb_mcp::call` (registry-only). So the loop reaches
**host-native** verbs (`agent.memory.*`, `assets.*`, `series.*`, `query.*`, …) AND extension tools
behind the identical `authorize_tool` + per-verb caps wall, under the derived principal
(`agent_caps ∩ caller.caps`). `skill.activate` stays the loop-internal built-in intercepted in `run.rs`
BEFORE `run_calls`, so it never reaches the bridge (unchanged). `call_tool` requires `&Arc<Node>`
(the wasm-guest callback bridge clones it), so the agent path was threaded `&Node → &Arc<Node>` along
`run_session`/`run_calls`/`resume_suspensions`/`invoke`/`resume`/`invoke_via_runtime`/`serve::run_one`/
`drive_run`/`drive_queued_run`/`triage` and the `AgentRuntime::run` trait (+ the external `AcpRuntime`
and the test stubs). Every call site already held an `Arc<Node>` (or `&Arc<Node>` via `&node`), so the
change is the parameter types only — `&node.store`/`&node.bus` uses deref-coerce unchanged.

**Surface the tools to the loop** (`crates/host/src/agent/menu.rs`, new): `reachable_tools(node,
principal, ws)` builds the loop's `AllowedTool` menu from `tools.catalog` (registry + host-native
descriptors ∩ the caller's grants). The channel worker (`channel/agent_worker.rs::drive_run`) now
passes `reachable_tools(...)` instead of the empty `&[]`. The wall re-checks every proposed call, so
the menu is not a widening (absent from the menu ⇒ also denied if proposed).

**Wire the model + boot** (`node/src/agent.rs`, new — the thin role-aware mount, like
`federation.rs`/`control_engine.rs`): reads the in-house model config from env (`LB_AGENT_MODEL_*`,
the `ModelEndpoint` shape — provider / model / api-key-env NAME / base-url), builds the in-house model
via `build_in_house_model` (real `AiGateway<Provider>` when an adapter exists; else `UnconfiguredModel`,
the honest empty state), builds the registry (in-house default + external `AcpRuntime` entries when the
`external-agent` feature is on, via the refactored `external_agent::register_external`),
`install_runtimes`, then `serve_agent(node, node.runtimes(), agent_caps())`. Mounted from
`node/src/main.rs` AFTER the gateway installs its signing key (the federation/control-engine ordering)
so a served run's tool callbacks verify. The old `external_agent::install` (which built its own
`UnconfiguredModel` registry) is gone; `node` now depends on `lb-role-ai-gateway` (not feature-gated —
the in-house default is present on every node). The key is an env NAME resolved through the adapter's
secrets path — never compiled in or logged.

## Decisions & alternatives (scope open questions, resolved)

- **Model endpoint config shape** → a small node-local `InHouseModelConfig` mirroring `ModelEndpoint`
  (provider/model/api_key_env/base_url), read from `LB_AGENT_MODEL_*`. Rejected reusing the
  feature-gated `lb-external-agent::ModelEndpoint` directly: the in-house model must be present
  feature-OFF too, so the config type can't live behind the `external-agent` feature. Same *shape*, one
  door, symmetric.
- **Where boot builds + installs the model** → a dedicated `node/src/agent.rs` mount module
  (mirroring `control_engine.rs`), called after the gateway key install. Rejected inlining into
  `main.rs` — one responsibility per file, and the ordering is load-bearing.
- **Tool-menu policy** → the full reachable catalog for v1 (the wall re-checks anyway). Note: the
  catalog is the *palette descriptor set* (`tools/descriptor.rs`) ∩ grants, not the entire host-native
  surface — so it is already a curated, bounded menu, not a raw dump. A curated per-workspace subset is
  a documented follow-up if context tax bites.
- **In-house model reaches the provider directly** (in-process `ModelAccess`), no double-hop through
  the served OpenAI face — that face is for external agents only (#4), deliberately not built here.
- **`agent.config` in-house-model reporting** → deferred as a UI-only additive field; not needed for
  the wiring. Left in the scope's follow-ups.
- **Threading `Arc<Node>` vs a `&Node` variant of `call_tool`** → thread `Arc<Node>`. Rejected teaching
  `lb_mcp::call` to fall through to host verbs (duplicates the routing `call_tool` centralizes) and
  rejected a `&Node`-only host-native dispatcher (the loop must also reach extension tools, whose guest
  callback genuinely needs the `Arc`). One bridge, one wall, one dispatch path for both agents.

## Tests

New `crates/host/tests/agent_in_house_wiring_test.rs` (rule 9: real `mem://` store, bus, caps,
gateway, loop; the ONLY fake is `MockProvider`). Uses `agent.memory.set` (workspace scope, so the
written row is identity-independent and directly assertable through the full wall incl. the
`store:agent_memory/workspace:write` gate) as the host-native verb whose effect proves execution:

- **the_in_house_loop_executes_a_host_native_tool_through_call_tool** — the headline: `AiGateway<MockProvider>`
  → `runtime:"default"` invoke → scripted model proposes `agent.memory.set` → the loop EXECUTES it
  through `call_tool` (was `NotFound` via the old registry-only path) → the memory row is in the store
  + the transcript records the proposed call and an OK result.
- **a_host_native_call_the_intersection_forbids_is_denied_and_fed_back** — capability-deny (§2.1): agent
  holds the write cap, caller does NOT → `agent ∩ caller` lacks it → the call is Denied, fed back as a
  tool error (loop still completes), nothing persists. A configured model grants no tool authority.
- **a_ws_b_run_cannot_reach_ws_a_memory_through_the_loop** — workspace-isolation (§2.2): a ws-B run's
  dispatch cannot reach a ws-A memory row (`call_tool` is workspace-first).
- **unconfigured_returns_the_honest_answer_then_configured_runs_the_loop** — the swap: `UnconfiguredModel`
  returns `UNCONFIGURED_ANSWER` + no tool effect; after `install_runtimes` with the real model the same
  invoke runs the loop and writes the row. The seam is the registry, not a code branch.
- **the_loop_menu_equals_the_callers_reachable_catalog** — the loop's `AllowedTool` menu equals
  `tools.catalog` for the caller; a tool the caller lacks the cap for is absent from BOTH.
- **an_external_run_reaches_a_host_native_verb_through_the_same_wall** + its deny twin — external-agent
  parity: an `AgentRuntime` that dispatches via `call_tool` under `agent ∩ caller` reaches a
  host-native verb through the same wall (and is denied identically when the intersection forbids it).
- **a_resumed_run_redrives_a_host_native_call_through_the_new_dispatch** — offline/sync (§2.3): a
  mid-run-disconnected durable session resumes and re-drives a host-native call through the new
  dispatch cleanly (rehydrate + the new dispatch compose; the pre-disconnect turn survives untouched).

Green output:

```
running 8 tests
test a_resumed_run_redrives_a_host_native_call_through_the_new_dispatch ... ok
test unconfigured_returns_the_honest_answer_then_configured_runs_the_loop ... ok
test the_loop_menu_equals_the_callers_reachable_catalog ... ok
test a_ws_b_run_cannot_reach_ws_a_memory_through_the_loop ... ok
test the_in_house_loop_executes_a_host_native_tool_through_call_tool ... ok
test a_host_native_call_the_intersection_forbids_is_denied_and_fed_back ... ok
test an_external_run_reaches_a_host_native_verb_through_the_same_wall ... ok
test an_external_run_host_native_call_is_denied_when_the_intersection_forbids_it ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.44s
```

**Regression — the full agent suite** (routing the loop through `call_tool` must not double-gate or
change deny/policy/skill.activate semantics). All green:

```
agent_config_test         6 ok    agent_offline_test        2 ok
agent_decision_test       7 ok    agent_rehydrate_test      3 ok
agent_default_runtime_test 5 ok   agent_routed_test         3 ok
agent_in_house_wiring_test 8 ok   agent_runtime_seam_test   5 ok
agent_isolation_test      2 ok    agent_runtimes_test       5 ok
agent_memory_test         8 ok    agent_skill_test          5 ok
agent_test                4 ok    agent_watch_test          4 ok
channel_agent_worker_test 8 ok    tools_catalog_test        4 ok
```

`cargo build --workspace` green (default + `--features external-agent`); `cargo fmt --check` clean.

Note on the full-workspace run: two classes of failures are **pre-existing and unrelated** to this
change — (a) tests that require a prebuilt wasm guest component (`proof_panel_test`,
`github_bridge_normalize_test`, …) panic with "missing component … build it first" until the ext's
`build.sh` runs; they pass once built. (b) `control_engine_appliance_routing_test` is a flaky
in-process **Zenoh two-node discovery** timing test (fails ~half its runs on the clean baseline too,
verified by stashing this change) and touches zero agent code — not introduced here.

## Debugging

None — nothing broke non-trivially. The two "failures" met during the full-suite run were a missing
prebuilt wasm artifact and a pre-existing Zenoh two-node flake (both verified against the clean
baseline), so neither warranted a `debugging/` entry.

## Public / scope updates

- Promoted to `public/agent/agent.md` (the finished in-house default: model door + shared tool wall +
  boot) and added a row to `public/SCOPE.md`.
- Scope open questions resolved in this doc (see Decisions); scope marked shipped.

## Skill docs

`docs/skills/agent/SKILL.md` (new) — drives the in-house default end to end: configure the model,
`agent.invoke` with no `runtime`, watch it call a platform tool (`agent.memory.set`) under the wall.
Grounded in the live `the_in_house_loop_executes_a_host_native_tool_through_call_tool` run (the exact
path was dead before this session).

## Dead ends / surprises

- The scope's example imagined the loop writing *member*-scope memory; but inside the loop the derived
  principal is `agent:session`, so member scope resolves to `member:agent:session` — a caller reading
  back under its own sub sees nothing. The write *worked*; the test used **workspace** scope (shared,
  identity-independent) to assert the effect cleanly, which also exercises the extra ws-write gate.
- `tools.catalog` is NOT the full host-native surface — it lists the curated palette descriptors
  (`tools/descriptor.rs`) ∩ grants. So the loop's menu is already bounded, not a raw host-verb dump.
  The loop can still EXECUTE any granted host-native verb the model proposes (via `call_tool`) even if
  it's not on the palette menu — the menu is a hint, the wall is the authority.

## Follow-ups

- Real AI-gateway provider adapter (ai-gateway scope) — then the boot path answers with a real LLM
  with zero change here (drop an arm into `build_in_house_model`).
- Optional curated/bounded agent tool subset if the reachable-catalog context tax bites.
- Optional additive `agent.config` "in-house model: configured/unconfigured" read field for the UI.
- The pre-existing `control_engine_appliance_routing_test` Zenoh two-node discovery flake (separate).
