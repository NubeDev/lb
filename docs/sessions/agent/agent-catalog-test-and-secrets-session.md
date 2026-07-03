# Agent-catalog test-and-secrets build session

Scope: `docs/scope/agent/agent-catalog-test-and-secrets-scope.md`. Built on `master`.

Two gated additions to the shipped agent catalog, reusing shipped seams (no new machinery):
1. a **"Test" button** that runs a context-proving diagnostic (`agent.def.test`);
2. a **DB-sealed per-workspace model key** (reference a `lb-secrets` path from an endpoint, resolve it
   at model-call time secret → env).

## What shipped

### 1. `agent.def.test {id?}` — the context-proving diagnostic

- **`crates/host/src/agent/defs/test.rs`** — `agent_def_test(node, caller, ws, id?)`. Gated
  `mcp:agent.def.test:call` (its own admin-tier cap — the test spends a model turn; opaque `Denied`).
  It resolves the target (the given id, or the active `agent.config` pick), assembles the caller's
  **real** run context — the shipped `SYSTEM_PROMPT`, `reachable_tools` (the MCP/ACP tool surface),
  and `render_catalog` (granted skills) — and runs **one** turn (step ceiling 1, no tool execution)
  over `node.runtimes().default_model()`. Returns `TestResult { id, answer, runtime, model, context:
  { tool_count, tools, skill_count, skills }, provider_configured, ok }`.
- **The wall is inherited, not widened.** `reachable_tools`/`render_catalog`/`list_granted_skills` are
  ws- + grant-gated for the caller — the test sees exactly what a real run for that caller would.
- **No key echo by construction.** The endpoint key is resolved out-of-band for the model transport,
  never injected into the prompt/context — so the returned `answer` is structurally key-free.
- **Bounded.** No durable session/transcript is persisted (the derived session id `{ws}:agent-def-test`
  is a per-turn idempotency key, not a job).
- Dispatch: an `agent.def.test` arm in `crates/host/src/tool_call.rs` (needs the `&Arc<Node>` the
  dispatcher holds). Gateway: `test_def` + `test_active_def` in
  `role/gateway/src/routes/agent_defs.rs`, routes `POST /agent/defs/test` and
  `POST /agent/defs/{id}/test`.

### 2. DB-sealed per-workspace model key

- **`crates/host/src/agent/defs/model.rs`** `DefinitionEndpoint` (and
  `crates/host/src/agent/config/model.rs` `ModelEndpointPatch`) gained a names-only optional
  **`api_key_secret: Option<String>`** — a **secret PATH** (e.g. `agent/<id>-key`), never a value.
- **`crates/host/src/agent/resolve_key.rs`** — `resolve_endpoint_key(store, principal, ws,
  secret_path, env_name)`: the ONE place an endpoint's key is resolved, precedence **sealed secret
  (`lb_secrets::get`) → node env → `None`**. Best-effort on the secret (a denied/absent path falls
  through to the env, never an error). Exported through `agent/mod.rs` + `lib.rs`; the swap-point
  comment in `node/src/agent.rs::build_in_house_model` names it as the sanctioned resolver so
  "test passes" and "run works" can't diverge.
- Caps: `credentials.rs` dev grants extended with `mcp:agent.def.test:call`, `mcp:secret.set:call`,
  `secret:agent/*:write`.

### UI (`ui/src/features/settings/agent/`)

- `agentDef.api.ts` — `api_key_secret`, `TestResult`/`TestContext`, `testAgentDef(id?)`,
  `setModelKey(path, value)` (via `secret.set`, `visibility: "private"` — value written once, never
  read back; only the path is stored on the definition).
- `useAgentTest.ts` + `AgentTestButton.tsx` — the per-entry Test button; shows the reply + a compact
  "context: N tools, M skills" line + an honest `provider_configured` note.
- `AgentCatalog.tsx` — `canTest` prop, renders `<AgentTestButton>` per runnable entry.
- `AgentDefinitionEditor.tsx` — a write-only **"Model key"** field: seals via `setModelKey`, stores
  only the path `agent/<id>-key`; on re-edit shows "key is set ✓ · rotate" (no readback).
- `AgentTab.tsx` — passes `canTest = hasCap(caps, CAP.agentDefTest)`; `CAP.agentDefTest` added.
- **`ActiveModelKey.tsx` + `useAgentCatalog.setActiveKey`** — a **"Set model key" / "Rotate key"**
  affordance on the ACTIVE pick (gated by `canPick` = `agent.config.set`). It seals the token via
  `secret.set` and writes only the resulting path onto **`agent.config`** — so an admin can key a
  read-only **built-in** they picked WITHOUT cloning it (scope open-question #5; `ModelEndpointPatch`
  already carried `api_key_secret`). `config.api.ts`'s `ModelEndpointPatch` gained `api_key_secret`.
  This closes the actual UX gap ("the in-house model is read-only — how do I add my token?").

## Tests (rule 9 — everything real; the model is the sanctioned external via `AiGateway<MockProvider>`)

- **`crates/host/tests/agent_def_test_test.rs`** (10 tests, all green): capability-deny (opaque);
  the test returns the caller's real assembled context (a granted skill + a reachable tool are named);
  inherits-the-wall (fewer grants → fewer skills; ws-B never lists ws-A's); sealed key names-only (the
  record holds only the path, the answer + DTO are value-free); `resolve_endpoint_key` precedence
  (secret → env → none, all three, incl. an absent path falling through); ws-isolation of the key
  (ws-B can neither `secret.get` nor resolve ws-A's); built-in write with a secret path is rejected
  (`BadInput`); bounded (no durable run record); `provider_configured` honest (`UnconfiguredModel` →
  false; a real `AiGateway` → true).
- **`ui/src/features/settings/AgentCatalogTestAndKey.gateway.test.tsx`** (3 tests, real spawned
  gateway, no `*.fake.ts`): the Test button runs the real diagnostic and shows the reply + context
  line + the honest "no model provider is wired" note (the test node runs `UnconfiguredModel`); the
  Model-key field seals a real secret and a fresh `agent.def.get` shows only the path (names-only, no
  value on the record), and the re-opened editor shows "key is set ✓" without readback; a built-in has
  no Model-key field (read-only tier — no editor opens).
- Also fixed two pre-existing `DefinitionEndpoint`/`ModelEndpointPatch` literals in
  `agent_defs_test.rs` for the new `api_key_secret` field.

## Commands (green)

- `cd rust && cargo build --workspace` and `cargo build --workspace --features external-agent` — green.
- `cargo test -p lb-host --test agent_def_test_test --test agent_config_test --test agent_defs_test
  --test agent_in_house_wiring_test --test agent_skill_test --test rules_ai_wiring_test` — all pass.
- `cargo test -p lb-role-gateway` — pass. `cargo fmt` — clean.
- `cd ui && pnpm test:gateway AgentCatalogTestAndKey` — 3/3 pass.

## Decisions (and rejected alternatives)

- **One turn with real context, not a bare ping.** A ping proves the endpoint resolves; it does not
  prove the agent knows what it is. Assembling + returning the real context is the only thing that
  answers "does it know it has MCP/ACP/skills" against the mock **and** a real provider.
- **The key is a reference, never inlined** (rejected: a plain column on the record — breaks names-only
  §6.7). The value flows only through the shipped sealed `secret.set`; the record carries the path.
- **One shared `resolve_endpoint_key` helper** (rejected: separate resolution in the test vs. the run —
  would let "test passes" diverge from "run works"). Both consume the one helper.
- **`provider_configured` is honest.** The test node runs `UnconfiguredModel`, so the UI says "no model
  provider is wired — the answer is a placeholder"; the **context line** is what makes the test
  meaningful pre-adapter, exactly as the scope's Risks section calls for.

## Known limits (unchanged by this slice)

No real `Provider` adapter exists yet (ai-gateway-deferred); the test proves the pipe + context
assembly + key resolution for real against `AiGateway<MockProvider>`, and the model's *understanding*
of the context is only truly demonstrated once a real adapter lands. Nothing is faked to hide this.
