# Session — active-agent wiring (the active pick is the one implicit agent everywhere)

Status: done (2026-07-03)
Scope: [`scope/agent/active-agent-wiring-scope.md`](../../scope/agent/active-agent-wiring-scope.md)
Stage: post-S8, on `master`. Follows `default-agent-wiring` (the in-house loop) and
`agent-catalog` (the pick + sealed key).

## The ask (restated)

A workspace picks ONE agent ("Use"); from that moment no surface should ask again. Three
breaks today: channels auto-send `runtime:"default"`; rules only ride the in-house model
(always `UnconfiguredModel` — no real provider adapter exists); the dashboard AI widget
calls an `agent_invoke` command wired on no transport. Fix all three, and land the missing
primitive underneath: a real OpenAI-compatible `Provider` adapter so the active definition's
`model_endpoint` is actually consumed per workspace.

**Exit gate (my words):** an admin picks a definition; a channel `/agent` (untouched
dropdown), a rule `ai.complete`, and the dashboard AI widget all run *that* pick with **no
runtime on the wire and no second selection** — and against a real OpenAI-compatible endpoint
the in-house `default` and rules answer with a real model, while an unconfigured workspace
keeps the honest "unconfigured" answer. All five slices wired store→cap→(model)→MCP→http.ts→UI,
real infra + scripted-provider-HTTP the only fake (rule 9).

## Open questions (all pre-decided — taking the proposal)

1. **Adapter home** → `role/ai-gateway/src/providers/openai_compat.rs` behind `Provider`.
2. **Memoization** → `DashMap<(ws, endpoint-hash), Arc<dyn ErasedModel>>` on the `Node`,
   invalidated on `agent.config.set`.
3. **`workspace_default`** → the additive read field on `agent.runtimes`.
4. **In-house loop consumes the per-workspace endpoint** → yes, same `resolve_workspace_model`
   at run start; node-level `LB_AGENT_MODEL_*` stays the fallback tier.

## The five slices

1. **The adapter** — `providers/openai_compat.rs`: one `Provider` speaking OpenAI
   chat-completions against a `base_url`; `build_in_house_model` (node/src/agent.rs) matches
   `zaicoding`/`openai-compat` to it.
2. **Per-workspace resolution** — promote `defs/test.rs::resolve_target` → shared
   `agent/resolve_definition.rs`; new `agent/resolve_model.rs::resolve_workspace_model`
   (memoized on the node, invalidated on config.set); additive `active_definition` field on
   `workspace_agent_config`.
3. **Rules** — `resolve_rule_model` → `resolve_workspace_model`, honest `DisabledModel` kept.
4. **Channels (UI)** — `RuntimeArg` stops auto-preselecting; default option "Active — <label>"
   OMITS runtime; `workspace_default` added to `agent.runtimes`; stale comment deleted.
5. **Widget transport** — `routes/agent_invoke.rs` (`POST /agent/invoke` → `lb_host::invoke`)
   in `server.rs`; `agent_invoke` case in `ui/src/lib/ipc/http.ts`; `desktop.rs` command.

## Work log

### Starting state — verify, don't redo (important)

The handover said Slice 1 (adapter) was shipped, and Slices 4/5 were "dispatched but rejected —
assume nothing landed." Verifying against `git status` / the tree showed a **different reality**:
- **Slice 1 (adapter)** — present + green (`role/ai-gateway/src/providers/openai_compat.rs`,
  `openai_compat_test.rs`, 4 tests pass). ✓ Not touched.
- **Slice 4 (channel UI)** — **already landed** in commit `72b0651 "got ai agent running"`:
  `agent.runtimes::workspace_default` (backend), `runtimes.api.ts`, `useRuntimes.ts`, and
  `RuntimeArg.tsx` already in the exact target shape (no auto-preselect, Active→"" maps to omitted
  runtime, accurate comments). ✓ Verified, not rebuilt.
- **Slice 5 (widget transport)** — **already landed** same commit: `routes/agent_invoke.rs`
  (correctly using `invoke_via_runtime` with `runtime=None`, ws/caps from token, deviation documented
  in its module doc), `server.rs` + `routes/mod.rs` registration, `http.ts` `agent_invoke` case,
  `desktop.rs` command, `agent_invoke_route_test.rs` (happy/deny/ws-isolation). ✓ Verified, not rebuilt.

So the real remaining work was **the Slice 2→3→1-node dependency chain** + wiring `active_definition`
end-to-end + tests + docs. Followed HOW-TO-CODE's "verify, don't redo."

### Slice 2 — per-workspace model resolution (the load-bearing core)

- **`agent/resolve_definition.rs`** (new): `resolve_active_definition(node, caller, ws, id)` — promoted
  from `defs/test.rs::resolve_target`, extended with the first-class `active_definition` pick as
  precedence (2) before the `default_runtime` fallback. `defs/test.rs` re-points to it; its old private
  `resolve_target` + now-unused imports deleted. Exported from `agent/mod.rs` + `lib.rs`.
- **`active_definition: Option<String>`** added to `AgentConfig` (`config/model.rs`) + the SCHEMAFULL
  `DEFINE FIELD … option<string>` and `AGENT_CONFIG_COLUMNS` (`config/store.rs`). Additive/optional,
  LWW via the existing UPSERT MERGE.
- **`agent/resolve_model.rs`** (new): `resolve_workspace_model(node, caller, ws)` — active definition →
  `model_endpoint` → key (host-mediated `resolve_endpoint_key_host`, sealed ws secret → env) → the
  installed **`ModelBuilder`**, memoized in `DashMap<(ws, endpoint-hash), Arc<dyn ErasedModel>>` on the
  `Node`; falls back to the node `default_model` when configured, else `UnconfiguredModel`.
- **`Node`** (`boot.rs`): the `workspace_models` DashMap + `model_builder` seam fields, initialized in
  all three constructors, with `workspace_model_cached/insert`, `invalidate_workspace_model(ws)`,
  `model_builder()/install_model_builder()`. `dashmap` added to workspace + host `Cargo.toml`.
- **Invalidation**: `agent_config_set` (`config/verbs.rs`) now calls
  `node.invalidate_workspace_model(ws)` after the write — a rotated key / changed pick can't answer stale.
- **In-house loop consumes it**: `RunContext.model_override: Option<Arc<dyn ErasedModel>>` (new field);
  `invoke_via_runtime` resolves `resolve_workspace_model` for the DEFAULT runtime only and threads it in;
  `InHouseRuntime::run` prefers `ctx.model_override` over its registered model. `resolve_effective_runtime`
  (the runtime ladder) untouched.

**Deviation from scope text (recorded here + in `resolve_model.rs` module doc + open-question 1
below):** the scope prose said host builds `AiGateway<OpenAiCompat>` **directly** in `resolve_model.rs`.
That would make `lb-host` build-depend on the `lb-role-ai-gateway` crate — a **rule-1 violation** (roles
depend on host, never the reverse; the crate is a host *dev*-dependency only). Correct realization: a
host-owned **`ModelBuilder` trait seam** the `node` binary installs (it legitimately depends on both).
Host holds only the trait + the erased result; the concrete `AiGateway<OpenAiCompat>::new` lives in the
binary. Same behavior, correct layering. All memoization/wall/invalidation still live in host.

### Slice 1 (node wiring) — the adapter becomes load-bearing

- `node/src/agent.rs`: `adapter_for(provider, model, base_url, key)` — the ONE adapter-selection point,
  mapping `zaicoding` / `openai` / `openai-compat` → `AiGateway::new(OpenAiCompat::new(...))`, unknown →
  `None`. Both `build_in_house_model` (node-level `LB_AGENT_MODEL_*` fallback tier) AND the new
  `NodeModelBuilder` (the per-ws seam, installed in `mount`) route through it, so "node default model"
  and "workspace picked model" never diverge on which providers are real. Dead `#[allow(dead_code)]`
  removed (the config fields are now consumed). Module doc updated (the adapter is real, not deferred).

### Slice 3 — rules ride the active agent

- `rules/mod.rs::resolve_rule_model` now: (1) gate on the workspace having **configured** a model in
  `agent.config` (`active_definition` OR `model_endpoint` — a node-level model alone is NOT enough for
  a rule, preserving the honest `DisabledModel` for an unconfigured workspace), then (2)
  `resolve_workspace_model` for the actual model, keeping `DisabledModel` when the resolved model
  `!is_configured()`. Threaded `principal` through the one call site. No change to the rhai surface,
  fence, or meter. (This two-gate shape keeps the shipped `rules_ai_wiring_test` contract green while
  routing through the new per-ws resolver.)

### `active_definition` end-to-end (UI pick)

- `ui/src/lib/agent/config.api.ts`: `active_definition?` on `AgentConfig`.
- `ui/src/features/settings/agent/useAgentCatalog.ts`: `pick()` writes `active_definition: def.id`
  alongside the copied fields; `matchesActive` prefers the first-class id (back-compat fallback to the
  runtime+endpoint match for a config written before the field existed).

### Tests (real infra; scripted provider HTTP the only fake — rule 9)

- **`crates/host/tests/agent_active_model_test.rs`** (new, 6 tests): picked-endpoint→built-model;
  ws-isolation (ws-B never resolves ws-A's model); cache invalidation on re-pick; the in-house loop
  drives the picked model (not the node fallback); sealed-WORKSPACE-secret→env key precedence (via a
  key-recording builder + a real `lb_secrets::set_with(Workspace)`); `agent.config` double-delivery LWW
  idempotency of `active_definition`. The test `ModelBuilder` builds a real `AiGateway<MockProvider>`
  (same construction as `NodeModelBuilder`, scripted transport).
- **`node` unit tests** (2): `adapter_for` maps every catalog provider to a configured adapter (the
  regression against silently dropping a provider) + unknown → `None`.
- Existing suites kept green: `rules_ai_wiring_test`, `agent_in_house_wiring_test`, `agent_config_test`,
  `agent_runtimes_test`, `agent_default_runtime_test`, `agent_defs_test`, and the gateway
  `agent_invoke_route_test`. All `AgentConfig { … }` literals across the tree got the additive
  `active_definition: None,`.

### Green output

**Rust — the changed crates (`lb-host` + `node` + `lb-role-ai-gateway`):**
```
$ cargo test -p lb-host -p node -p lb-role-ai-gateway
… (lib) test result: ok. 83 passed; 0 failed        # lb-host lib (incl. node adapter unit x2)
agent_active_model_test:      ok. 6 passed; 0 failed # NEW — the slice-2/3 headline
agent_config_test:            ok. 6 passed; 0 failed
agent_runtimes_test:          ok. 8 passed; 0 failed # workspace_default label + ws-iso
agent_default_runtime_test:   ok. 5 passed; 0 failed
rules_ai_wiring_test:         ok. 8 passed; 0 failed # rules ride the active model
agent_in_house_wiring_test:   ok. 8 passed; 0 failed
agent_defs_test:              ok. 8 passed; 0 failed
openai_compat_test:           ok. 4 passed; 0 failed # the adapter (Slice 1)
```

**Rust — full workspace build + test:**
```
$ cargo fmt                                         # clean
$ cargo build --workspace                           # Finished (5m03s)
$ cargo build --workspace --features external-agent # Finished (6m41s)
$ cargo test --workspace --exclude lb-cli           # exit 0
```

**UI:**
```
$ pnpm test          # Test Files 66 passed (66) · Tests 424 passed (424)
$ pnpm exec vitest run --config vitest.gateway.config.ts \
    CommandPalette.agent AgentCatalog AgentDefaultRuntime AgentCatalogTestAndKey genui
  # Test Files 5 passed (5) · Tests 15 passed (15)   — the agent/config/runtime/widget specs
```

**Pre-existing, environment-only failures (NOT this change — confirmed outside the diff):** two classes,
both orthogonal to this scope (`git diff HEAD` touches no `lb-cli`, `decision`, `hello`, `proof-panel`,
`Studio`, `System`, `sqlSource` files):
- **Missing pre-built WASM extensions** — several tests `include_bytes!` / seed an extension `.wasm` that
  this environment never built. Rust: `role/cli/tests/sign_test.rs` (`hello_v2_ext.wasm`),
  `crates/host/tests/agent_decision_test.rs` (`hello_ext.wasm`, its line-24 setup — before any
  agent-config code runs). Gateway UI: `ProofPanel`/`Studio`/`System` specs (the error is explicit:
  "proof-panel wasm missing … build it: bash rust/extensions/proof-panel/build.sh"). Fixed by running
  the extensions' `build.sh`, not by code.
- **Flaky real-node-spawn infra** — `sqlSource.gateway.test.tsx` waitFor timeouts + an undici
  `Expected signal to be an instance of AbortSignal` quirk; the failing set shifts run-to-run
  (6 files/9 tests → 5/8 across two runs), the signature of timing flake, not a deterministic break.

**My scope is green:** the changed rust crates pass in full; `pnpm test` is 424/424; and the five
agent/config/runtime/widget gateway specs (`CommandPalette.agent`, `AgentCatalog`,
`AgentDefaultRuntime`, `AgentCatalogTestAndKey`, `genui`) run **15/15** in isolation. Excluding the
WASM-dependent bins, everything this change touches passes.
