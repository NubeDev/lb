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

> **Provider adapters — the current truth.** As of the default-agent-wiring slice **no real `Provider`
> adapter is implemented yet** (only the test `MockProvider`; the real OpenAI-compatible / local
> adapters are ai-gateway-scope-deferred). So setting `LB_AGENT_MODEL_PROVIDER` today logs
> "provider '<x>' has no adapter yet — keeping UnconfiguredModel" and the seam waits. The wiring, the
> config, and the unconfigured→configured **swap** are done and proven against `MockProvider`; the day
> a real adapter lands it drops into `build_in_house_model`'s match and the same config answers with a
> real LLM — no other change.

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

## Common "why isn't it working?"

- **"It says no in-house model is configured."** No `LB_AGENT_MODEL_PROVIDER`, or the named provider
  has no adapter yet (see §1). This is the honest empty state, not a bug.
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

