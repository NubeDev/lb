# Agent (public)

The central, workspace-scoped AI agent — shipped at S5. The ask lives at
`../../scope/agent/agent-scope.md`; the build session at `../../sessions/agent/ai-core-session.md`.

## What it is

A **host service** (`lb_host::agent`, beside `channel`/`assets`) that hosts a workspace-scoped actor
which **owns the tool-call loop**. It is not a model and not a wasm extension: the loop must call
`caps::check` on each tool dispatch, read S4 assets through the host verbs, and drive a durable job —
all host-internal seams.

## The loop (the agent is the loop)

Bounded by `MAX_STEPS`:

1. ask the model for a turn via the **gateway** (`ModelAccess::turn`) — replay-safe by a per-step
   idempotency key;
2. for each proposed tool call, run it through the ONE host MCP bridge **`call_tool`** (the same entry
   the gateway's `POST /mcp/call` uses) under the **derived** principal — so the loop reaches
   **host-native** verbs (`agent.memory.*`, `assets.*`, `series.*`, `query.*`, …) AND extension tools,
   each capability-checked, workspace-first, routed if the tool is on another node. A denial is fed
   back to the model, not a crash. (`skill.activate` is the loop-internal built-in, intercepted before
   dispatch.) Before the default-agent-wiring slice this went through `lb_mcp::call` (extension
   registry only), so a proposed host-native verb `NotFound`ed — see "The finished in-house default";
3. persist the step to the job (idempotent, append-addressed) and advance the cursor;
4. repeat until the model is done or the ceiling is hit; then `complete` the job.

The gateway does **model access only** — keeping the loop out of it is what lets the gateway be a
swappable sidecar (`ai-gateway/ai-gateway.md` / the ai-gateway scope).

## The intersection (no widening)

The agent acts under `agent ∩ caller`:

- `Principal::derive(sub, agent_caps)` mints a strictly **narrower** actor — same workspace
  (delegation can't cross the wall), a distinct `agent:*` sub (audit shows the agent acted), the
  agent's caps, and the caller's caps as a `constraint`.
- `caps::check` runs **gate 2b**: a delegated request must match the caller's caps too. Exact set
  intersection, no pattern algebra — an agent can never do something *either* side forbids.
- **Substrate** (granted skill + shared doc) is read **on the caller's behalf**: the caller's
  identity resolves the S4 membership/grant gate 3, the intersected caps bound the capability gate.
  (See `../../debugging/agent/agent-reads-doc-it-doesnt-own-is-denied.md` for why.)

## Reachability — MCP + routed

`agent.invoke` is a host-native MCP tool (the same `lb_mcp::authorize_tool` bridge as `assets.*`):
the MCP gate (`mcp:agent.invoke:call`, workspace-first) then the loop. An **edge** invokes the
**hub** agent over the routed namespace — `invoke_remote` authorizes on the edge, then `query`s the
hub's queryable (`ws/*/agent/invoke`, the S3 routing seam). `caps::check` runs on the calling node;
the workspace-scoped key keeps isolation structural.

## The session is a durable job

The agent's session is an `lb-jobs` `job:{id}` record (workspace-scoped, no separate datastore): an
**append-addressed transcript** + a cursor. The edge that invokes does not hold the session — if it
disconnects mid-loop, the hub keeps running and each step persists. `resume` re-reads the record and
continues from the cursor; re-applying a persisted step is a no-op, and the gateway's idempotency
cache means the resumed turn is not re-spent — so resume is idempotent.

## Tested (the S5 mandatory categories)

- **Capability-deny:** the invoke gate (no `mcp:agent.invoke:call` → refused before the loop); the
  in-loop intersection (a tool the *caller* lacks is denied even if the agent holds it — no
  widening); an ungranted substrate skill is invisible.
- **Workspace-isolation:** an agent in workspace B can never read workspace A's docs/skills/jobs —
  across store + MCP, and across the routed edge→hub path.
- **Offline/sync:** a session interrupted mid-loop resumes from its cursor (no double-apply); a
  duplicated invocation does not re-spend the gateway or duplicate a step.

## The finished in-house default (default-agent-wiring)

The always-registered `"default"` runtime is now a *working, platform-native* agent, wired by four
seams (the ask: `../../scope/agent/default-agent-wiring-scope.md`; session:
`../../sessions/agent/default-agent-wiring-session.md`):

- **A real model door.** The node builds the in-house `ModelAccess` from config (`LB_AGENT_MODEL_*`,
  the `ModelEndpoint` shape — provider / model / api-key-env **NAME** / base-url) and installs it via
  `Node::install_runtimes`, so the in-house `default` binds the real `AiGateway<Provider>` instead of
  the placeholder. **No model configured → `UnconfiguredModel`** (the honest empty state — returns
  "no in-house model configured", not a fake). The swap is the *registry*, never an `if cloud`. The
  provider key is an env NAME resolved through the secrets path — never compiled in or logged.
- **The shared tool wall.** The loop dispatches through `call_tool` (loop step 2 above), so the
  default agent can call the platform's own host-native verbs AND extension tools through the identical
  `authorize_tool` + caps wall — under `agent ∩ caller`. **This same fix serves the external agent**:
  its tool bridge rides the one dispatch, so both fronts are platform-capable with no second path.
- **The loop's menu = the reachable catalog.** Callers surface the caller's reachable
  `tools.catalog` (registry + host-native descriptors ∩ grants) to the loop as its `AllowedTool` list.
  The wall re-checks every call, so the menu is a *hint*, not a widening — a tool absent from the menu
  is also denied if proposed.
- **Boot.** The node builds the registry (in-house default over the wired model + external entries when
  the `external-agent` feature is on) and calls `serve_agent`, mounted after the gateway installs its
  signing key (the federation/control-engine ordering). So a routed `agent.invoke` and the in-channel
  `/agent` path both drive the finished agent.

**Note on the model provider:** as of this slice **no real `Provider` adapter exists** (only the
sanctioned test `MockProvider`; the real adapters are ai-gateway-scope-deferred). The wiring seam,
config, and unconfigured→configured **swap** are shipped and proven for real against
`AiGateway<MockProvider>`; the day a real adapter lands it drops into `build_in_house_model` with no
other change and the in-house agent answers with a real LLM.

## Agent catalog (shipped)

A **library of named agent definitions** — each a `(runtime, model_endpoint)` preset — that a
workspace admin manages and picks from. Two tiers, one record shape (the core-skills pattern, reused
wholesale):

- **Built-ins** — six presets boot-seeded from an embedded `agents.toml` manifest into the reserved
  `_lb_agents` namespace, **read-only to users** (a `builtin.*` id rejects create/update/delete with
  `BadInput`, checked before the caps gate). Ships in-house (runtime `default`) and Open Interpreter
  (runtime `open-interpreter-default`) × Z.AI **GLM-4.6 / 5.1 / 5.2** over the `zaicoding` coding
  endpoint (`ZAI_API_KEY`). Names only — no secret values in the manifest or a record. A node without
  the `external-agent` feature still seeds the open-interpreter entries but **filters them from the
  list** (registry drift, symmetric — no `if cloud`). Idempotent re-seed on boot/upgrade.
- **Custom** — workspace-scoped `agent_definition` records with full admin CRUD (custom-only writes;
  the workspace hard wall). LWW UPSERT on the slug (offline-replay-safe).

**Verbs** (`crates/host/src/agent/defs/`, one per file): `agent.def.list` / `agent.def.get` (member),
`agent.def.create` / `update` / `delete` (admin, custom-only). Gateway routes mirror them
(`GET|POST /agent/defs`, `GET|PATCH|DELETE /agent/defs/{id}`). A custom write validates its `runtime`
against the node registry (an unrunnable id is `BadInput`, the shipped `agent.config.set` rule).

**Selection reuses the shipped seam.** Picking a definition writes `agent.config.set { default_runtime,
model_endpoint }` from its fields, so `resolve_effective_runtime` honors the choice with **no new
resolution path**. The catalog is the library; `agent.config` stays the one active selection. The
Settings → **Agent** tab is the catalog manager: built-ins read-only, custom editable, active pick
highlighted.

**Honest limit:** picking sets the workspace default **runtime** today (the invoke path honors it);
routing the in-house loop to a **per-workspace endpoint** waits on the ai-gateway provider adapter
(below). The UI copy says so. The ask lives at `../../scope/agent/agent-catalog-scope.md`; the build
session at `../../sessions/agent/agent-catalog-session.md`.

## Not yet (follow-ups)

A real model **provider adapter** behind the gateway contract (the in-house model door + boot are now
wired; only the concrete `impl Provider` is deferred — the swap point is `build_in_house_model`);
streaming progress as Zenoh motion + the durable transcript via the outbox; token-on-the-bus for
routed invocations (S5 is in-process co-trust); an optional curated/bounded agent tool subset if the
reachable-catalog context tax bites; an optional additive `agent.config` "in-house model:
configured/unconfigured" read field for the UI; per-workspace loop policy. The coding workflow that
composes the agent (issue → triage → approval → job → outbox) is S6.
