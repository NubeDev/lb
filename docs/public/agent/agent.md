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

## Testing a definition + sealing its model key

Two gated additions to the catalog (`../../scope/agent/agent-catalog-test-and-secrets-scope.md`;
`../../sessions/agent/agent-catalog-test-and-secrets-session.md`):

**`agent.def.test {id?}` — the context-proving diagnostic** (`crates/host/src/agent/defs/test.rs`,
gated `mcp:agent.def.test:call`, its own admin-tier cap — it spends a model turn). It resolves the
target (the given id, or the active `agent.config` pick), assembles the caller's **real** run context
exactly as `run.rs` does — `SYSTEM_PROMPT` + `reachable_tools` (the MCP/ACP tool surface) +
`render_catalog` (granted skills) — and runs **one** turn (step ceiling 1, no tool execution) over the
node's `default_model`. Returns `{ answer, runtime, model, context: { tool_count, tools, skill_count,
skills }, provider_configured, ok }`. The **context line** ("context: N tools, M skills") is what proves
the agent *was given* its Lazybones context even against the deterministic mock; `provider_configured`
is honest (false on the `UnconfiguredModel` placeholder — the UI never implies a real LLM answered). The
test inherits the wall (context is the caller's own ws- + grant-gated surface — never widened, never
another tenant's) and is bounded (no durable session/transcript persisted). The key is resolved for the
model transport out-of-band, never injected into the prompt — so the answer is structurally key-free.
Gateway: `POST /agent/defs/{id}/test` (and `POST /agent/defs/test` for the active pick).

**DB-sealed per-workspace model key.** The endpoint gains a names-only optional **`api_key_secret`** —
a **secret PATH** (e.g. `agent/<id>-key`) into `lb-secrets`, beside `api_key_env`. Neither is a value.
`api_key_secret` lives on BOTH `DefinitionEndpoint` (a custom definition) and `ModelEndpointPatch` (the
active `agent.config` pick), so there are **two ways to add a token in the UI**:
- **On the active pick — including a built-in — without cloning it.** The active entry carries a
  write-only **"Set model key" / "Rotate key"** affordance; it seals the value via `secret.set` and
  writes only the path onto `agent.config` (the active selection is workspace-scoped and can own a
  sealed secret path — scope open-question #5). This is the self-serve "key the read-only in-house
  model" path: you key your *selection*, not the built-in record.
- **On a custom definition — the editor's write-only "Model key" field.** Seals on save; a re-edit
  shows "key is set ✓ · rotate"; never a readback.

At model-call time the one shared `resolve_endpoint_key` (`crates/host/src/agent/resolve_key.rs`)
resolves **sealed secret (`lb_secrets::get`) → node env → unset** — the SAME resolver the test and a
real run both consume, so "test passes" and "run works" can't diverge. A `builtin.*` *record* stays
read-only + node-env (its write is rejected); the sealed key rides the workspace's `agent.config`
selection instead. Names-only holds by construction: the value lives only in `lb-secrets`, never on a
record, manifest, or log — the tests assert the record/config + the returned answer are value-free.

## The active pick is the ONE implicit agent everywhere (active-agent-wiring)

A workspace picks ONE agent ("Use") and **no surface asks again**. This closes the three breaks the
pick used to have and lands the primitive under them — a real provider adapter consumed per workspace.

**The adapter (the unblock).** `role/ai-gateway/src/providers/openai_compat.rs` — one `Provider`
speaking the OpenAI **chat-completions** shape against a configurable `base_url` (covers `zaicoding`,
`openai`, and any `openai-compat` server). Honest-failure contract: any network/non-2xx/unparseable
fault → a terminal `AiResponse::stop` attributed `"model call failed: openai-compat/{model}: …"`, never
a silent empty answer, never a panic, key never logged. The node's `adapter_for` (`node/src/agent.rs`)
maps those providers to it; the in-house `default` is now honest, not a de-facto mock.

**`active_definition` is first-class.** `agent.config` gains one additive optional `active_definition`
id, written by the pick alongside the copied endpoint fields. The shared
`resolve_active_definition` (`crates/host/src/agent/resolve_definition.rs`, promoted out of
`agent.def.test`) is the single answer to "which definition is active" for the UI badge, rules, and the
test button: explicit id → `active_definition` → `default_runtime` (as id, then as a runtime match).

**Per-workspace model resolution.** `resolve_workspace_model(node, caller, ws)`
(`crates/host/src/agent/resolve_model.rs`) resolves the active definition's `model_endpoint` → a live
model, keyed by the host-mediated **sealed-workspace-secret → node-env** key
(`resolve_endpoint_key_host`), **memoized** in a `DashMap<(ws, endpoint-hash), Arc<dyn ErasedModel>>` on
the `Node` and **invalidated on `agent.config.set`** (a rotated key / re-pick never answers stale). It
falls back to the node-level `LB_AGENT_MODEL_*` model, else the honest `UnconfiguredModel`. `lb-host`
never build-depends on the gateway crate: the concrete `AiGateway<OpenAiCompat>` is built by a
host-owned **`ModelBuilder`** seam the `node` binary installs (rule 1 — the resolver/cache/wall stay in
host; only the `new()` lives in the binary).

**Every consumer rides it, implicitly.**
- **Channels** — `RuntimeArg`'s default entry is "Active — <label>" and sends **no** `runtime`; the
  shipped `resolve_effective_runtime` fallback runs the pick. `agent.runtimes` gained an additive
  `workspace_default` so the dropdown labels without a second fetch. Picking a concrete id stays an
  explicit per-message override.
- **The in-house loop** — an implicit `runtime:default` run drives the workspace's picked model via a
  per-run `RunContext.model_override` (`resolve_workspace_model` at run start), node env as fallback.
- **Rules** — `resolve_rule_model` rides the same resolver: a workspace that configured a model gets a
  real `ai.*`; one with no pick keeps the honest "AI not configured for rules". (A node-level model
  alone is NOT enough for a rule — the workspace must have chosen.)
- **The AI widget** — `POST /agent/invoke` (`role/gateway/src/routes/agent_invoke.rs`) drives
  `invoke_via_runtime` with `runtime=None` (workspace + caps from the token, never the body), so the
  genui author flow resolves the active agent. Browser `agent_invoke` case in `http.ts`; Tauri command
  in `desktop.rs`.

The wall holds throughout: `active_definition` + the resolved model live on the workspace-scoped
`agent.config`; a ws-B rule/widget can never resolve ws-A's endpoint or key (the DashMap key carries
`ws`, the secret read is namespace-walled). No new caps or tables.

## Not yet (follow-ups)

> The close-out ask for this list lives at `../../scope/agent/agent-close-out-scope.md`: real token
> accounting, per-workspace loop policy, run progress as bus motion + outbox completion, and
> token-on-the-bus are its four slices; the rest below is deferred to its owning topic (named there).

Streaming progress as Zenoh motion + the durable transcript via the outbox; token-on-the-bus for
routed invocations (S5 is in-process co-trust); an optional curated/bounded agent tool subset if the
reachable-catalog context tax bites; provider fallback chains + the served OpenAI face for external
agents (`model-routing-scope` #4); a real per-provider token count surfaced on `Turn` (rules' budget
meter currently estimates from content length); per-workspace loop policy. The coding workflow that
composes the agent (issue → triage → approval → job → outbox) is S6.
