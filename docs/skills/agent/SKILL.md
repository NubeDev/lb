---
name: agent
description: >-
  Configure and drive the IN-HOUSE default agent (the always-registered `"default"` runtime) end to
  end. Use when a task says "use the built-in/in-house agent", "configure the agent's model", "wire a
  model for the default agent", "make the agent use a platform tool", "why does the agent answer 'no
  model configured'", "agent.invoke with no runtime", or "get the default agent to call
  agent.memory/series/assets". Covers the node model config (`LB_AGENT_MODEL_*`, the ModelEndpoint
  shape — provider/model/api-key-env NAME/base-url), the unconfigured→configured swap, the shared
  `call_tool` dispatch that lets the loop reach host-native verbs under `agent ∩ caller`, the reachable
  tool menu from `tools.catalog`, and boot (`serve_agent`). For the OPT-IN third-party ACP runtimes
  (Open Interpreter / VT Code / Codex), see `../external-agent/SKILL.md` instead.
---

# Driving the in-house default agent end to end

The **default runtime is the in-house loop** (`runtime:"default"`, or omit `runtime`). This skill is
its operating manual: give it a model, invoke it, and watch it call a platform tool under the wall.

> **The one thing that decides "does it answer or say unconfigured?"**: whether the node has an
> in-house **model** installed. No model → the honest `UNCONFIGURED_ANSWER`. A model → the loop runs.
> Selecting the default runtime needs **no** extra grant (it's the absent-runtime default); invoking
> needs `mcp:agent.invoke:call`; each tool the agent then calls is re-checked under `agent ∩ caller`.

Everything below is grounded in a live run: `crates/host/tests/agent_in_house_wiring_test.rs`
(`the_in_house_loop_executes_a_host_native_tool_through_call_tool`) drives a real `AiGateway<MockProvider>`
as the in-house model, `agent.invoke` with no runtime, the model proposes `agent.memory.set`, and the
loop **executes it through `call_tool`** — the memory row lands in the store.

---

## 1. Configure the model (node config)

The in-house model is node config, read at boot into the `ModelEndpoint` shape (names only — the key
is an env **NAME**, never a value):

```
LB_AGENT_MODEL_PROVIDER=<provider id>      # e.g. openai (names the adapter)
LB_AGENT_MODEL_MODEL=<model id>            # e.g. gpt-4o
LB_AGENT_MODEL_API_KEY_ENV=<ENV VAR NAME>  # e.g. OPENAI_API_KEY — the NAME; the value stays in env
LB_AGENT_MODEL_BASE_URL=<optional base>    # OpenAI-compatible endpoint
```

At boot `node/src/agent.rs` reads these, builds the real `AiGateway<Provider>` for the named provider,
installs it as the in-house `default`, and calls `serve_agent`. **No provider set → the in-house
`default` stays `UnconfiguredModel`** — the honest empty state, symmetric on every node (no `if cloud`).

> **Provider adapters — the current truth (active-agent-wiring, shipped).** The real adapter is **live**:
> `adapter_for` (`node/src/agent.rs`) maps `zaicoding` / `openai` / `openai-compat` to a real
> `AiGateway<OpenAiCompat>` — the OpenAI chat-completions wire shape
> (`role/ai-gateway/src/providers/openai_compat.rs`). So `LB_AGENT_MODEL_PROVIDER=zaicoding` +
> `…_MODEL=glm-4.6` + `…_API_KEY_ENV=ZAI_API_KEY` answers with a real LLM. An **unknown** provider still
> logs "has no adapter" and keeps `UnconfiguredModel` (the honest empty state). A model call that fails
> (network / non-2xx / unparseable) returns an attributed terminal stop —
> `"model call failed: openai-compat/<model>: …"` — never a silent empty answer, never a panic, key never
> logged. The node-level `LB_AGENT_MODEL_*` model is the **fallback tier**; a *workspace* that picked an
> agent in the catalog rides its OWN endpoint per run (next section).

## 2. Confirm the runtime is offered

`agent.runtimes` lists the node's registered runtimes; `"default"` is always present (the in-house
loop). A node built without `--features external-agent` offers *only* `"default"`.

## 3. Invoke it — no runtime

```json
// mcp/call → agent.invoke   (needs mcp:agent.invoke:call, workspace-first)
{ "goal": "what's the latest reading on boiler-1, and remember it runs hot" }
// runtime omitted → the in-house default
```

or in a channel, post a `kind:"agent"` item with no `runtime` (the `/agent` palette). The run is a
durable job; watch it live over `agent.watch` (SSE). With **no** model configured the run returns
`UNCONFIGURED_ANSWER` immediately and proposes no tools.

## 4. Watch it call a platform tool under the wall

The loop is handed the caller's **reachable** tool menu (`tools.catalog` ∩ the caller's grants), so
the model has real tools to propose — e.g. `series.latest`, `agent.memory.set`. Each proposed call is
dispatched through the ONE host bridge `call_tool` (the same entry `POST /mcp/call` uses) under the
**derived** principal `agent ∩ caller`:

- a tool the intersection **allows** → executed against the platform (the effect lands: a memory row,
  a series read, a doc write);
- a tool the intersection **forbids** → `Denied`, fed back to the model as a tool error (not a crash —
  the model can react), nothing happens.

**Model access grants no tool authority.** Configuring a model does not let the agent do anything it
couldn't already; the wall re-checks every call. A run under user U reaches exactly `U's caps ∩ the
agent's caps` — and the `member:{user}` memory scope is U, structurally never another user.

### The wall, concretely

| The caller can… | …then the loop can propose it, and it | The menu |
|---|---|---|
| run `agent.memory.set` (holds `mcp:agent.memory.set:call`) | **executes** — the row lands | present |
| NOT run `assets.put_doc` (lacks the cap) | is **Denied**, fed back | absent |
| reach only ws-B | can never touch ws-A data (workspace-first) | ws-B only |

## 5. Where it runs

`serve_agent` declares the routed `agent/invoke` queryable at boot, so an **edge** `agent.invoke`
reaches this node's finished agent over the workspace-scoped bus key (isolation is structural). The
in-channel `/agent` worker drives the same installed registry. Symmetric: any node can host the agent;
a solo node simply has no remote callers.

---

## 6. The active pick is the ONE implicit agent — everywhere (active-agent-wiring)

Once a workspace picks an agent in Settings → Agent ("Use"), **no surface asks again**. The pick writes
`agent.config { active_definition, default_runtime, model_endpoint }`; from there every consumer that
isn't given an *explicit* override resolves that pick — and rides its `model_endpoint` per workspace
(not just the node-level `LB_AGENT_MODEL_*`). Drive them all implicitly:

- **A channel** — post `/agent <goal>` **without touching the runtime dropdown** (it reads
  "Active — <label>"). The payload carries **no** `runtime`; the worker resolves the active pick and
  streams back. (Grounded: `agent_default_runtime_test`,
  `ui …/CommandPalette.agent.gateway.test.tsx` — an untouched palette posts no `runtime`.)
- **A rule** — `ai.complete("summarize", grid)` reaches the workspace's active model through
  `resolve_workspace_model`; a workspace with no pick keeps the honest "AI not configured for rules".
  (Grounded: `rules_ai_wiring_test`.)
- **The dashboard AI widget** — a `genui` cell → `agent_invoke` → `POST /agent/invoke` (workspace +
  caps from the token, never the body; `runtime=None`) → the active agent authors the widget. (Grounded:
  `role/gateway/tests/agent_invoke_route_test.rs`.)

How "active" resolves (one shared seam, `resolve_active_definition`): explicit id → `active_definition`
→ `default_runtime` (as an id, then as a runtime match). The model behind it
(`resolve_workspace_model`) is: the active definition's `model_endpoint` → the adapter, keyed
**sealed-workspace-secret → node-env**, memoized per `(ws, endpoint)` and **invalidated on re-pick**;
else the node fallback model; else the honest `UnconfiguredModel`. The wall holds: ws-B never resolves
ws-A's endpoint or key. (Grounded: `crates/host/tests/agent_active_model_test.rs`.)

> **A quick live check.** With `LB_AGENT_MODEL_*` unset and no pick, `agent.invoke {goal}` returns
> `UNCONFIGURED_ANSWER` and `ai.complete` errors "AI not configured for rules". Pick a `zaicoding`
> definition (seal `ZAI_API_KEY` or set the env), and the SAME channel post / rule / widget answer with
> the real model — no runtime named anywhere. Re-pick a different model and the next run reflects it
> (the cache invalidated).

---

## Common "why isn't it working?"

- **"It says no in-house model is configured."** No workspace pick AND no `LB_AGENT_MODEL_PROVIDER`
  (or the named provider is unknown to `adapter_for` — `zaicoding`/`openai`/`openai-compat` are the
  known ones; see §1). This is the honest empty state, not a bug. Pick an agent (§6) or set the node
  env to wire a model.
- **"The agent proposed a tool but it was denied."** The caller (or `agent ∩ caller`) lacks that
  tool's cap. The menu shows what the caller may run; the wall re-checks. Grant the cap to the caller.
- **"I want a third-party coding agent, not the in-house loop."** That's the opt-in external runtime —
  see `../external-agent/SKILL.md` (compile-time `external-agent` feature + `runtime:` selection).

---

## Testing a definition + setting its model key (token)

Both surfaces are agent-/API-drivable over the gateway (rule 7), so this skill covers them.

### Test a definition — `agent.def.test {id?}`

A gated diagnostic (`mcp:agent.def.test:call`, admin-tier — it spends a model turn) that runs **one**
turn with the caller's **real** assembled context (system prompt + reachable MCP/ACP tools + granted
skills) so you can confirm the agent has its Lazybones context:

```
POST /agent/defs/{id}/test        # a specific definition
POST /agent/defs/test             # the active agent.config pick (no id)
# → { answer, runtime, model, context: { tool_count, tools, skill_count, skills },
#     provider_configured, ok }
```

Read the **context line** ("context: N tools, M skills"): it proves the agent *was given* its tools +
skills even against the deterministic mock. `provider_configured` is honest — `false` when the node
runs the `UnconfiguredModel` placeholder (no provider adapter wired), so the answer is a placeholder,
not a real LLM.

### Set / rotate the model key (the token) — sealed, DB-backed

There **is** a way to add a token — it is a **sealed workspace secret**, not a field on the definition
record (names-only, §6.7). The flow is two shipped verbs; the UI's "Model key" field does both for you:

1. **Seal the value** through the shipped sealed secret store:
   ```
   POST /mcp/call  { "tool": "secret.set",
                     "args": { "path": "agent/<id>-key", "value": "<THE-TOKEN>",
                               "visibility": "private" } }
   ```
   The value lands **only** in `lb-secrets` (owner-stamped, workspace-scoped, never logged, never read
   back to the browser). Needs `mcp:secret.set:call` + `secret:agent/*:write`.
2. **Reference the path** (a name, never the value) on the definition:
   ```
   POST /agent/defs   { …, "model_endpoint": { …, "api_key_secret": "agent/<id>-key" } }
   ```

At model-call time the key resolves **sealed secret → node env (`api_key_env`) → unset** via the one
shared `resolve_endpoint_key`. So: a workspace that sealed a key uses it; one that only set a node env
var keeps working off the env.

**Two UI paths to add a token (Settings → Agent):**
- **On the active pick — including a built-in — no clone needed.** Pick the model, then use the
  **"Set model key" / "Rotate key"** affordance on the active entry. It seals the token via `secret.set`
  and writes only the resulting path onto **`agent.config`** (the active selection is workspace-scoped
  and can own a sealed secret path — scope open-question #5). This is the answer to "the in-house model
  is read-only, how do I give it my token?": you key your *selection* of it, not the built-in record.
- **On a custom definition — the editor's write-only "Model key" field.** Seals on save; shows
  "key is set ✓ · rotate" thereafter; never reads the value back.

Both never show the value back and store only the path (names-only).

