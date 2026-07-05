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


## 7. Personas — pick *what the agent is for* (agent-personas)

A run today is handed everything: the constant system prompt, your whole reachable tool catalog, no
grounding. A **persona** fixes that — it is a workspace-selected *focus*: `{ identity, granted_tools,
grounding_skills, extends }`, applied at run assembly to **narrow** the run. It never widens the wall.

> **The one rule:** a persona **narrows, never widens.** The effective menu is
> `persona ∩ agent ∩ caller`, and every dispatch still re-runs `caps::check`. A persona listing a tool
> you lack changes nothing (still denied); a granted tool the persona omits is just un-advertised (a
> model that proposes it anyway still hits the wall). Persona = advertisement + grounding + supervision,
> **not** authorization. Picking a persona never grants a capability.

### The record (two tiers, one shape — the `agent.def` pattern)

- **Built-in** — `builtin.<slug>`, seeded read-only into the reserved `_lb_personas` namespace
  (readable in every workspace, writable in none; a `builtin.*` write is `BadInput` before the caps gate).
- **Custom** — a workspace-scoped record with admin CRUD.

```
Persona {
  id, label, description?,
  identity: string,          // prepended to the in-house SYSTEM_PROMPT / folded at the head of the goal
  granted_tools: string[],   // tool ids or trailing-* globs ("flows.*") — OPAQUE data, a narrowing hint
  grounding_skills: string[],// skill ids pinned at session start (grant-gated, FAIL-CLOSED)
  extends: string[],         // parent persona ids; tool/skill lists union in at read (child identity wins)
  policy_preset?, runtimes?, // #4 (extension-builder): a supervision floor + a runtime restriction
}
```

### Pick one (member reads, admin writes)

```
# The picker read (member): list built-ins ∪ your workspace's custom personas
POST /mcp/call { "tool": "agent.persona.list", "args": {} }        # → { personas: [...] }
POST /mcp/call { "tool": "agent.persona.get",  "args": { "id": "builtin.data-analyst" } }

# Set the workspace's active persona (rides agent.config, the active_definition move exactly)
POST /mcp/call { "tool": "agent.config.set", "args": { "patch": { "active_persona": "builtin.data-analyst" } } }
```

Precedence at run assembly: an **explicit** per-invoke `persona` arg > the workspace `active_persona` >
none. A per-message override rides every front door — the channel `kind:"agent"` payload, the routed
`agent.invoke`, and `POST /agent/invoke { …, "persona": "builtin.flow-author" }` (so a surface like Data
Studio can invoke with its own focus, no new mechanism). An explicit **unknown** id is a named error (an
explicit ask must not silently degrade); a **dangling active** id (persona since deleted) warns and runs
un-narrowed — never an errored run.

### The seven built-in personas (pick by name)

Seeded read-only into `_lb_personas`. Each curates one platform area's verbs + pins ≤ 4 grounding
skills (the rest ride the filtered catalog). **Every tool still passes the wall** — an admin-tier verb
in a persona does nothing for a member caller. **Destructive/security verbs (`workspace.delete/purge`,
`authz.revoke-tokens`, `secret.get`) are excluded from every persona by design** — a human runs those.

| Persona | For | Pinned grounding |
|---|---|---|
| `builtin.data-analyst` | datasources, SurrealDB, series, queries, charts | `core.datasources`, `core.query`, `core.store-read`, `core.ingest-series` |
| `builtin.flow-author` | the typed-node flows DAG engine | `core.flows-mcp`, `core.ingest-series`, `core.query` |
| `builtin.widget-builder` | Data Studio, charts, GenUI, render templates | `core.dashboard-mcp`, `core.genui-widget`, `core.panels`, `core.dashboard-widgets` |
| `builtin.rules-author` | rules — **extends** flow-author + data-analyst | `core.rules` (+ inherited) |
| `builtin.workspace-admin` | nav, users, teams, roles, grants, ws defaults | `core.nav`, `core.auth-caps`, `core.prefs` |
| `builtin.channels-operator` | channels, inbox/outbox, messaging | `core.channels-inbox-outbox`, `core.prefs` |
| `builtin.system-manager` | the general operator — **extends** all six above; hands off deep work | `core.lb-cli`, `core.mcp`, `core.auth-caps`, `core.agent` |

`rules-author` and `system-manager` use `extends` — they inherit their parents' tools + skills at read
time, so when a parent grows (e.g. new `flows.*` verbs) they follow for free, no seed edit.

> **What "narrow the menu" reaches today:** the run's menu is the palette-descriptor catalog
> (`tools.catalog`) + loaded extension tools — a curated subset, not the full ~175-verb surface. A
> persona's `granted_tools` is the complete forward-looking allow-list (it narrows correctly as verbs
> gain descriptors / arrive as extension tools). On a bare node the tool-narrowing is small, so the
> **identity + pinned grounding** carry most of the confusion cure there; tool-narrowing bites hardest
> with many extension tools loaded.

### Author a custom persona (admin)

```
POST /mcp/call { "tool": "agent.persona.create", "args": {
  "id": "my-analyst", "label": "My analyst",
  "identity": "You are a data analyst for this workspace. Verify against the store; never invent columns.",
  "granted_tools": ["federation.query", "viz.query", "series.*", "store.query", "store.schema"],
  "grounding_skills": ["core.query", "core.store-read"],
  "extends": []
} }                                                                # → { ok: true }
POST /mcp/call { "tool": "agent.persona.update", "args": { "id": "my-analyst", "patch": { "label": "Renamed" } } }
POST /mcp/call { "tool": "agent.persona.delete", "args": { "id": "my-analyst" } }
```

Write-time walls (all `BadInput`): a `builtin.*` id (reserved, before caps); a bare `*` in
`granted_tools` (an everything-persona is *no* persona — leave it unset for no narrowing); a `*`
mid-string (globs are trailing-prefix only); a self- or cyclic `extends` chain, or one deeper than 3.

### What the run actually gets (both runtimes, one seam)

Applied in `invoke_via_runtime` (the ONE seam the in-house loop and the external ACP runtime share):

1. **menu** = `reachable_tools ∩ persona.granted_tools` (glob = trailing-`*` prefix). This narrowed
   `AllowedTool` list is the in-house model's menu **and** what the external MCP bridge advertises.
2. **identity** — the persona identity + each pinned `grounding_skills` **body** are folded into the
   goal (which seeds the in-house rehydrate and is the external agent's only channel). **FAIL-CLOSED:**
   an ungranted pinned skill fails the run at start with a named error, before any model spend.
3. **catalog** — the advertised skills catalog is filtered to the pinned set (the model sees the
   persona's focus). The grant is still the wall; filtering only removes already-granted entries.

### Inspect the effective focus (the Settings "effective tools" view)

```
POST /mcp/call { "tool": "agent.persona.resolve", "args": { "id": "builtin.data-analyst" } }
# → { effective: { id, identity, granted_tools, grounding_skills, policy_preset?, runtimes? } }
#   (the extends-closure UNION; the UI intersects granted_tools with the caller's tools.catalog to show
#    persona ∩ agent ∩ caller, with a reason per exclusion — "not in persona" / "not granted".)
```

### Supervision (the Allow/Ask/Deny policy — its first Settings surface)

The shipped per-tool policy sits **in front of** the wall (defense in depth; default-allow when unset):

```
POST /mcp/call { "tool": "agent.policy.get", "args": {} }                 # → { rules: [...] } (member)
POST /mcp/call { "tool": "agent.policy.set", "args": { "rules": [
  { "tool": "ext.publish", "effect": "ask" } ] } }                        # admin; Deny>Allow>Ask
```

A persona's optional `policy_preset` (the #4 extension-builder ships one) is the **floor**: tightening is
free, loosening below it is the explicit admin write.

> Grounded in a live run: `crates/host/tests/agent_persona_test.rs` seeds a record-only persona, drives a
> real in-house loop, and asserts the recorded menu is narrowed + the identity reached the model — and a
> scripted external runtime advertises the same narrowed set (`swap_test_*`). The pinned-but-ungranted
> skill test proves the fail-closed named error before any model call.
