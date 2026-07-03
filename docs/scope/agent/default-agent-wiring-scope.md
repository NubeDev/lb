# Agent scope — finishing the default agent (wire the in-house brain + shared tool dispatch + boot)

Status: **shipped** (2026-07-03) — see `../../sessions/agent/default-agent-wiring-session.md` and
`../../public/agent/agent.md` ("The finished in-house default"). All four seams built + green; open
questions resolved in the session doc. The only deferral is the real AI-gateway `Provider` adapter
(ai-gateway scope) — the wiring seam + config + unconfigured→configured swap are shipped and proven
against the sanctioned `MockProvider`; a real adapter drops into `build_in_house_model` with no other
change.

The in-house agent *engine* is built (the `run.rs` loop: caps-checked tool calls, durable resume,
approval gates, skill catalog, memory injection) and it is the always-registered `"default"` runtime
— **but it has no brain and can't reach the platform's own tools.** Today `runtime:"default"` binds
to `UnconfiguredModel` (returns "no in-house model is configured"), the agent loop dispatches proposed
calls through the extension **registry only** (so host-native verbs like `agent.memory.*`/`assets.*`
are unreachable), the channel worker hands the loop an **empty tool list**, and no boot path calls
`serve_agent`. This scope **finishes the default agent**: wire a real model into the in-house loop,
make the loop able to call host-native tools through the one capability wall (a fix that serves the
external agent too), surface the caller's reachable tools to the loop, and wire it all at node boot —
so an omitted `runtime` reaches a working, platform-native agent.

## Goals

- **A working in-house default.** `agent.invoke` with no `runtime` (or `runtime:"default"`) runs the
  real loop against a **real model**, wired at boot from node config — off (honest "unconfigured"
  answer) when no model is configured, symmetric on every node.
- **The default agent can use the platform's own tools.** The loop can call **host-native** MCP verbs
  (`agent.memory.*`, `assets.*`, `series.*`, `query.*`, …) and extension tools, each through the SAME
  `caps::check` wall under the derived principal (`caller ∩ agent`). This is the missing shared
  dispatch layer — it fixes the in-house loop AND is what the external agent's tool bridge rides on.
- **The loop is given the tools it may use.** Callers (the channel agent worker, `agent.invoke`)
  surface the caller's **reachable** tool set to the loop (via the existing `tools.catalog` gate) so
  the model actually has tools to propose — not the current empty list.
- **Boot wiring.** The node builds the runtime registry (in-house default over the real model +
  external entries when the feature is on) and calls `serve_agent`, so both the routed
  `agent.invoke` and the in-channel `/agent` path drive the finished agent. The serve-wiring `TODO`
  in `node/src/external_agent.rs` is closed.
- **External agent parity on tools.** With the shared host-native dispatch in place, the external
  agent's tool calls route through the same wall (its `granted_tools` reach host-native verbs too),
  closing the "neither agent can call platform tools" gap noted in the agent-memory session.

## Non-goals

- **Building the AI-gateway's real provider adapters (rig/OpenAI/local).** ai-gateway scope owns the
  provider adapter behind `ModelAccess` (it lists real adapters as "deferred past S5"). This scope
  **wires** whatever real `ModelAccess` the gateway exposes into the in-house runtime and proves the
  path with the sanctioned `MockProvider` at the test boundary; the day a real adapter lands it drops
  in behind the same `ModelAccess` with no change here. If no real adapter exists yet, the deliverable
  is the *wiring seam + config + the unconfigured→configured swap*, not a new provider.
- **The OpenAI-compatible served endpoint for external agents** — that is `model-routing-scope.md` (#4)
  and its blocking ai-gateway dependency. This scope does not build the served `/chat/completions`
  face; the external agent keeps its current transport. (The tool-wall fix here is independent of it.)
- **A new model/agent UI.** The Settings → Agent surface (runtime picker + endpoint) already ships
  (agent-config). This scope may extend it to show "in-house model: configured/unconfigured", nothing
  more.
- **Rebuilding a coding agent in-house.** The internal agent's tools stay *platform* tools; autonomous
  file/shell coding remains the external agent's job (its own toolset).
- **Mid-stream budget/streaming-token motion** — ai-gateway deferrals, unchanged.

## Intent / approach

**One agent engine, one model door, one tool wall — finish the wiring, don't fork anything.**

1. **Wire the model (the "push to get the internal one working").** `InHouseRuntime` already holds an
   erased `ModelAccess`; boot binds `UnconfiguredModel` today. Add a boot step that, **from node
   config** (a model endpoint: provider/model/`api-key-env` NAME + `base_url`, the `ModelEndpoint`
   shape agent-config/profiles already use), constructs the real `AiGateway<Provider>` as the node's
   `ModelAccess` and installs it via `Node::install_runtimes` — so `RuntimeRegistry::with_default`
   binds the real model instead of the placeholder. No model configured → keep `UnconfiguredModel`
   (the honest empty state, not a fake). Rejected: a compiled-in default key (scatters secrets, breaks
   local-only) — the key is an env NAME resolved through the secrets path, exactly as the external
   profile does.

2. **The shared host-native tool dispatch (the load-bearing fix).** Today `agent/step.rs::run_calls`
   calls `lb_mcp::call(&node.registry, …)`, which resolves against the **extension registry only** —
   a proposed `agent.memory.set`/`assets.put_doc` returns `NotFound`. Route the loop's tool calls
   through the host's **one MCP bridge** `tool_call.rs::call_tool` (the SAME entry the gateway's
   `POST /mcp/call` uses), which already dispatches host-native verbs AND extension/registry tools
   behind the identical `authorize_tool` + per-verb gate. So the loop reaches every tool a gateway
   caller can, under `caller ∩ agent`, with no second dispatch path. `skill.activate` stays the
   loop-internal built-in (it mutates run state) — unchanged. Rejected: teaching `lb_mcp::call` to
   fall through to host verbs (duplicates the routing `tool_call.rs` already centralizes; the bridge
   is the one seam).

3. **Surface the tools to the loop.** The caller assembles the `AllowedTool` list from the **reachable
   catalog** — `tools.catalog` already computes "every tool the caller may run" under the same
   `authorize_tool` gate (a denied tool is simply absent). The channel worker and `agent.invoke`
   populate the loop's `tools` from it (optionally filtered to an agent-appropriate subset), replacing
   the current `&[]`. The wall still re-checks every call, so the catalog is a *menu*, not a widening.

4. **Boot it.** The node builds the registry (default over the wired model + external entries when the
   feature is on) and calls `serve_agent(node, Arc::new(registry), agent_caps)` where the gateway is
   mounted (after the signing key is installed — the same ordering `federation`/`control_engine` use).
   The in-channel `/agent` worker already reaches `invoke_via_runtime`; it now passes a real tool list.

**Why this is the right shape:** the engine and the wall already exist and are shared. The only things
missing are *bindings* — model → runtime, loop → the one tool bridge, caller → the reachable tool
list, registry → boot. Each is a wiring seam, not a new subsystem. Fixing the tool bridge once makes
BOTH agents platform-capable, which is the honest answer to "why maintain two": the enforcement + tool
+ memory core is written once and both fronts call into it.

## How it fits the core

- **Tenancy / isolation:** unchanged and reinforced. Every tool the loop calls goes through
  `call_tool` → `authorize_tool`/`caps::check` **workspace-first**, under the derived principal. A
  ws-B run can only reach ws-B tools/data. The routed `serve_agent` key stays `ws/{caller.ws}/…`.
- **Capabilities (the deny path):** no new caller capability — invoking stays `mcp:agent.invoke:call`;
  choosing a runtime is an argument. The loop's reach is exactly `agent_caps ∩ caller.caps` — a tool
  the intersection forbids is `Denied` and fed back to the model, never executed. Deny-test: a run
  whose derived principal lacks `mcp:assets.put_doc:call` cannot write a doc even if the model proposes
  it. Model wiring adds **no** authority (model access is not a tool gate).
- **Placement:** `either`. The model endpoint is config: hub → shared/pooled provider; edge → a local
  provider or unconfigured. Symmetric — the "unconfigured vs configured" difference is config, never an
  `if cloud`. `serve_agent` runs on any node (a solo node just has no remote callers).
- **MCP surface:** **no new verbs.** This scope wires existing surfaces:
  - consumes `tools.catalog` (reachable tool list) to build the loop's menu;
  - routes loop tool calls through `call_tool` (the existing bridge);
  - `agent.invoke` / `agent.watch` / `agent.runtimes` / `agent.config.*` unchanged.
  Optionally a tiny **read** addition: `agent.runtimes` (or `agent.config.get`) reports whether the
  in-house `default` has a model configured (so a UI can show "unconfigured") — additive, one field,
  no new verb. **CRUD/live-feed/batch: N/A** (no new resource; the run's live feed is the shipped
  `agent.watch` SSE).
- **Data (SurrealDB):** none new. The model endpoint is node config (env/boot), not a tenant record;
  the run stays the durable job (`jobs`), the transcript the shipped transcript. State only.
- **Bus (Zenoh):** the shipped `agent/invoke` queryable (routed invocation) + the `RunEvent` feed —
  unchanged. `serve_agent` declares the queryable at boot (it currently isn't).
- **Sync / authority:** unchanged — the run is a hub-authoritative durable job; resume is the shipped
  rehydrate. Offline: an edge with no model configured returns the honest unconfigured answer.
- **Secrets:** the provider key is an **env NAME** in the model endpoint config, resolved through the
  secrets path (§6.7) — never a compiled-in or logged value, mirroring the external profile's
  `api_key_env`. The in-house model holds the key the same way the gateway does for any provider.
- **No fake backend (rule 9):** the store, bus, caps, gateway, and both runtimes are real. The ONE
  permitted fake is the **model provider HTTP** (`MockProvider`, behind the `Provider` trait) — tests
  wire the real `AiGateway` over `MockProvider` and drive the real loop + real `call_tool` dispatch, so
  the wiring (model → runtime, loop → host-native tool, boot registry) is exercised for real with no
  network. No `*.fake.ts`, no second dispatch path.
- **SDK/WIT impact:** none — host + role wiring only; the `AgentRuntime`/`ModelAccess`/`RunContext`
  seams are unchanged (this fills them in, doesn't move them).
- **Skill doc:** **yes** — update `docs/skills/external-agent/SKILL.md` (or a new
  `skills/agent/SKILL.md`) to document driving the **in-house** default end to end: configure the
  model, `agent.invoke` with no runtime, watch it call a platform tool (e.g. read a series / write
  memory) under the wall, grounded in a live run. The current external-agent skill only covers the
  external runtime.

## Example flow

1. A node boots with a model endpoint configured (`LB_AGENT_MODEL_*` / node config): boot builds the
   real `AiGateway` over that provider as the node's `ModelAccess`, installs the registry (in-house
   `default` over it + external entries if the feature is on), and calls `serve_agent`.
2. Ada (edge) asks the agent in a channel with no `runtime`: "what's the latest reading on `boiler-1`,
   and remember that it runs hot." The `/agent` worker resolves `runtime` → `default`, builds the tool
   menu from `tools.catalog` under Ada's grant (so `series.latest`, `agent.memory.set`, … appear),
   and drives the loop.
3. The loop asks the model; the model proposes `series.latest {series:"boiler-1"}`. The loop dispatches
   it through `call_tool` → `authorize_tool` (`caller ∩ agent`) → the host-native series verb → result
   fed back. Then the model proposes `agent.memory.set {scope:"workspace", slug:"boiler-1-runs-hot",…}`
   → same wall → persisted.
4. The model answers; the run completes as a durable job. Ada watches it live over `agent.watch` SSE.
5. On a node with **no** model configured, step 1 keeps `UnconfiguredModel`; step 3 never happens — the
   run returns the honest "no in-house model configured; select an external runtime or wire a model".
6. A run whose derived principal lacks `mcp:agent.memory.set:call` proposes the memory write → `Denied`,
   fed to the model as an error (the model can react); nothing is persisted. The wall held.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), all rule-9 (real store/bus/caps/gateway;
`MockProvider` is the one sanctioned external):

- **Capability-deny (§2.1):** a real run whose `agent_caps ∩ caller` lacks a tool's cap has that
  proposed call `Denied` and fed back (not executed) — proven for a host-native verb
  (`agent.memory.set` / `assets.put_doc`) reached through the new `call_tool` dispatch. Model access
  itself grants no tool authority (a configured model + no tool cap still denies the tool).
- **Workspace-isolation (§2.2):** a ws-B run cannot reach ws-A tools/data through the loop's dispatch
  (the `call_tool` gate is workspace-first); the routed `serve_agent` key is `ws/{caller.ws}/…`.
- **The wiring (the headline):** with a real `AiGateway<MockProvider>` installed as the in-house model,
  a `runtime:"default"` (or omitted) `agent.invoke` runs the loop, the scripted model proposes a
  **host-native** tool call, the loop **executes it through `call_tool`** (previously `NotFound`), and
  the run settles with the tool's effect visible in the store — the exact path that is dead today.
- **Unconfigured→configured swap:** with `UnconfiguredModel` the default returns `UNCONFIGURED_ANSWER`
  and proposes no tools; after `install_runtimes` with the real model the same invoke runs the loop —
  proving the seam is the registry, not a code branch.
- **Tool menu from the reachable catalog:** the loop's `AllowedTool` list equals the caller's reachable
  `tools.catalog` (a tool the caller can't run is absent from the menu AND denied if proposed).
- **External-agent parity:** an external run's tool call reaches a host-native verb through the same
  wall (its `granted_tools` include it) — the shared dispatch serves both.
- **Offline/sync:** the run stays a durable, resumable job (regression on the shipped resume — the new
  dispatch path must not break rehydrate); a mid-run disconnect resumes and re-drives cleanly.
- **Boot smoke (feature-on, opt-in / `#[ignore]`):** a node booted with a real endpoint lists the
  in-house `default` as configured, `serve_agent` is live, and a real invoke answers (provider key from
  env) — the live grounding the SKILL.md is written from.

## Risks & hard problems

- **No real provider adapter may exist yet.** ai-gateway lists real adapters as deferred. If so, the
  *model wiring* here is proven only against `MockProvider`, and "actually answers with a real LLM"
  waits on ai-gateway. Mitigation: make the deliverable the **seam + config + swap** (unconfigured →
  configured), so the day a real adapter lands it is a config change. State plainly in the session doc
  whether a real adapter was available.
- **Routing the loop through `call_tool` changes the dispatch path.** `call_tool` re-runs the MCP gate
  and reaches host-native + registry + routed tools; must confirm it composes with the loop's derived
  principal, the policy (Allow/Deny/Ask), and `skill.activate` interception without double-gating or
  changing deny semantics. Regression the full agent test suite.
- **Tool-menu size / context tax.** The reachable catalog can be large; a giant tool list per turn
  costs tokens and can confuse the model. Decide whether to inject the full reachable set or an
  agent-appropriate subset (open question). Don't silently truncate — if bounded, `log`/document it.
- **Budget/loop safety with a real model.** A real model + real tools can loop or spend. The shipped
  `MAX_STEPS` ceiling + policy gates apply; confirm they bound a *real* run, not just a scripted one.
- **Secret handling parity.** The in-house model's provider key must be mediated like every other
  secret (env NAME, never logged) — a leak here is a real credential leak, unlike the mock path.

## Open questions

**All resolved (2026-07-03) — decisions recorded in the session doc.** Summary:
- **Config shape** → a node-local `InHouseModelConfig` mirroring `ModelEndpoint` (`LB_AGENT_MODEL_*`),
  NOT the feature-gated external `ModelEndpoint` (the in-house model must exist feature-OFF too).
- **Where boot builds it** → a dedicated `node/src/agent.rs` mount module (mirroring `control_engine.rs`),
  called after the gateway key install.
- **Tool menu policy** → the full reachable `tools.catalog` for v1 (already curated to palette
  descriptors ∩ grants, so bounded); a per-workspace subset is a follow-up if context tax bites.
- **Direct provider vs served face** → in-house `ModelAccess` calls the provider directly (in-process);
  the served OpenAI face is external-only (#4), not built here.
- **`agent.config` in-house-model reporting** → deferred as a UI-only additive read field.

Original text (for the record):

- **Model endpoint config shape:** reuse the external `ModelEndpoint`
  (provider/model/`api_key_env`/`base_url`) as the node's in-house model config, or a dedicated
  `LB_AGENT_MODEL_*` set? Proposal: reuse `ModelEndpoint` (one shape, agent-config already renders it).
- **Where boot builds + installs the model:** in `node/src/main.rs` (beside `external_agent::install`)
  or inside the gateway mount (co-located with `serve_agent`)? Proposal: a `node/src/agent.rs` mount
  module (mirroring `control_engine.rs`), env-gated, called after the gateway key install.
- **Tool menu policy:** full reachable catalog vs. a curated agent tool set vs. per-workspace config?
  Proposal: full reachable catalog for v1 (the wall re-checks anyway), with a documented follow-up for
  a curated/bounded set if context tax bites.
- **Does the in-house model reach the provider directly, or through the gateway's own served face?**
  For v1 the in-house `ModelAccess` calls the provider adapter directly (it's in-process); the served
  OpenAI face is only for *external* agents (#4). Confirm no double-hop is intended.
- **`agent.config` "in-house model" reporting:** add a read field for configured/unconfigured, or infer
  it from `agent.runtimes`? Proposal: one additive field, no new verb.

## Related

- `agent-scope.md` (the engine this finishes), `agent-run/agent-run-scope.md` (the loop parts:
  policy/decision, skills, watch), `agent-memory/agent-memory-scope.md` (a tool the finished agent now
  reaches; the session doc names this exact dispatch gap).
- `external-agent/external-agent-scope.md` (umbrella), `runtime-seam-scope.md` (the registry seam this
  boots), `model-routing-scope.md` (#4 — the external served face this deliberately does NOT build),
  `capability-wall-scope.md` (the wall both agents ride; the shared dispatch is its in-house twin),
  `agent-config-scope.md` (the workspace default-runtime + the `ModelEndpoint` shape reused here).
- `ai-gateway/ai-gateway-scope.md` (owns the real provider adapter behind `ModelAccess` — the
  dependency for a *real* LLM answer).
- Skills: `../../skills/external-agent/SKILL.md` (extend/mirror for the in-house default), the
  new/updated `skills/agent/SKILL.md`.
- README `§6.16` (shared AI agents), `§6.14`/`§6.15` (gateway), `§6.9`/`§6.10` (jobs/durability),
  `§6.7` (secrets), `§7` (tenancy), `§3` (rules 1/5/6).
- Code the build touches: `crates/host/src/agent/{in_house,registry,serve,step,dispatch,run}.rs`,
  `crates/host/src/tool_call.rs` (the one bridge), `crates/host/src/tools/catalog.rs`,
  `crates/host/src/boot.rs` (`install_runtimes`), `node/src/{main,external_agent}.rs` (the serve-wiring
  TODO), `role/ai-gateway/*` (the `ModelAccess`/`Provider` seam).
```
