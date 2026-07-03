# Rules `ai.*` → real model wiring — session

- Date: 2026-07-03
- Scope: ../../scope/rules/rules-ai-wiring-scope.md
- Stage: post-S8 platform capability (rules plane, AI half); see STATUS.md
- Status: done (green) — the `DisabledModel` hardcode is gone from the configured `rules.run` path;
  a rule's `ai.*` now reaches the node's real `ModelAccess`, resolved per-workspace from `agent.config`.

## Goal

Fill the one open seam the rules engine shipped with: `rules_run(…, model: Arc<dyn RuleModel>, …)`
already took the model as a parameter, but the production bridge (`call_rules_tool` → `rules.run`)
hardcoded `Arc::new(DisabledModel)` (whose `complete`/`propose_sql` always `Err("AI not configured
for rules")`). So a rule's `ai.ask`/`ai.complete` failed over the gateway even when the workspace had
picked a model. This session **binds the rule engine's model seam to the real agent model** — a
`RuleModel` adapter over the host's `ModelAccess` (the AI-gateway surface the agent core uses),
resolved from the workspace's `agent.config` (the agent-catalog pick). The fence, meter, and cage are
**unchanged** — this only fills the model.

## What shipped (the scope's "one adapter + one wiring change")

1. **`crates/host/src/rules/model.rs` (new) — `AgentRuleModel`.** A `RuleModel` over the node's
   `ModelAccess` (held erased as `Arc<dyn ErasedModel>`). `complete(prompt)` and `propose_sql(q, hint)`
   each drive **one `ModelAccess::turn` with an EMPTY tools list** on the rule's blocking thread
   (`Handle::block_on` — the engine already runs under `spawn_blocking`), and read `turn.content`.
   Single-turn, no loop, no tool channel (a rule's power is the rule verbs, gated by the cage + caps).
   - **Tool-only turn is handled, never hangs:** a `turn` that returns `calls` with empty `content`
     is surfaced as a clear error ("returned only tool calls, no completion") rather than looping
     (there is no loop) — the scope's "single-turn vs. agent-loop drift" risk.
   - **Idempotency:** the adapter carries a per-run key prefix `(ws, rule|body-hash, run-ts)` and
     appends a monotonic index per `ai.*` call, so a re-run replays cleanly through the gateway's turn
     cache and two calls in one run don't collide.

2. **`crates/host/src/rules/mod.rs` — resolve the model on the bridge; retire the hardcode.**
   `resolve_rule_model(node, ws, idem)` returns `Arc::new(AgentRuleModel::new(model, ws, idem))` when
   **both** hold: (1) the workspace **selected** a `model_endpoint` in `agent.config` (the catalog
   pick, read via the host-internal `get_agent_config`), and (2) the node has a **real** provider
   (`ErasedModel::is_configured()` — not the `UnconfiguredModel` placeholder). Either missing →
   `DisabledModel` (now a *resolved* outcome, the honest `"AI not configured for rules"` error, not a
   hardcoded default). Data-only rules run regardless.

3. **Exposing the model to a non-agent caller (minimal support changes).**
   - `RuntimeRegistry` keeps `default_model: Arc<dyn ErasedModel>` (the same `Arc` the in-house
     `default` runtime runs) + `default_model()` accessor — so the rules bridge reaches "the workspace's
     model" (= the model its agent uses) without the whole `AgentRuntime` loop.
   - `ModelAccess::is_configured()` (default `true`) + forwarded through `ErasedModel` / `ModelHandle`;
     `UnconfiguredModel` overrides it to `false`. This is the honest "is there a real provider" signal
     the resolver reads through the erasure — no downcast, no second config axis.

## Testing (rule 9 — everything real; the model is the sanctioned external behind `Provider`)

New: `crates/host/tests/rules_ai_wiring_test.rs` — **8/8 green**, all over the REAL bridge
(`call_tool("rules.run")`) with a real `AiGateway<MockProvider>` installed as the node's model:

- **AI wired (headline):** a configured workspace's `ai.complete` returns the model's real
  (mock-deterministic) output (lands in `findings`); the meter records one AI call. Proves
  `DisabledModel` is gone from the configured path.
- **AI-not-configured:** a workspace that never selected a model runs a data-only rule fine; `ai.*`
  errors clearly. Plus **selected-but-no-provider** (a selected model on a placeholder-only node still
  errors — never fabricates).
- **Workspace-isolation:** one node, one installed model; ws-B selects, ws-A does not — the same rule
  errors in ws-A and answers in ws-B (the selection is per-workspace).
- **Fence holds (regression):** `ai.ask`'s model-proposed SQL is re-validated through
  `DataSeam::collect` — denied at collect without `store.query`, exactly as today.
- **Budget meter charges:** a rule making 9 `ai.complete` calls trips the default 8-call budget with a
  real model behind (asserted without mutating process-global env, which would race parallel tests).
- **Adapter unit:** `AgentRuleModel` maps `complete`/`propose_sql` onto `ModelAccess::turn` (no tools)
  over a scripted `ModelAccess`, driven on a blocking thread (as the engine drives it); a tool-only
  turn errors rather than hangs.

Regression sweep (green, no change): `rules_test` (7/7), `agent_in_house_wiring_test` (8/8),
`agent_runtime_seam_test` (5/5), `agent_default_runtime_test` (5/5), `agent_config_test` (6/6).

## Notes / gotchas encountered

- **`emit` takes a Map, not a string** — the emitted value lands in `findings` (`output` is just the
  `kind`). Tests assert against `findings`.
- **`block_on` from an async thread panics** — the adapter must be exercised on a blocking thread; the
  unit tests wrap it in `spawn_blocking` (mirroring the engine's `spawn_blocking`). `Handle::current()`
  is captured at construction on the async thread, used inside the blocking closure.
- **Inherited dirty tree** — the working tree already carried an in-flight, non-compiling
  `agent-catalog` (`defs/`) build (a `ModelEndpointPatch.api_key_secret` field + `agent.def.test` verb
  added mid-migration). Un-broke the shared suite where it was safe (added the missing
  `api_key_secret: None` to the stale `agent_config_test.rs`); left the actively-edited `defs/` files
  (`agent_defs_test.rs`, `agent/mod.rs`, `lib.rs` catalog exports) to that session.

## Known gap (named, not a lie)

- **Token accounting is an estimate, not the provider's count.** `Turn` carries no token field at the
  `ModelAccess` altitude (the gateway's `AiResponse.tokens` are dropped in the bridge), so the adapter
  estimates tokens from completion length so the meter still bites proportionally. Documented in
  `model.rs` as a KNOWN GAP: when `Turn` grows a real token field, return it instead — else the budget
  is only an approximation. The `MockProvider` returns a fixed count regardless, so this does not
  affect the tests; it matters when a real provider adapter lands (scope "Budget under a real model").
- **No real provider adapter yet** — proven against `AiGateway<MockProvider>`. The day a real
  `impl Provider` lands, a rule's `ai.*` answers with a real LLM with **no further rule change** (the
  seam resolves to `ModelAccess` either way). UI/docs copy stays honest about this (skill updated).

## Open questions (from the scope) — dispositions

- **A `rules.ai` sub-capability?** Not added in v1. `rules.run` already gates the whole surface; the
  budget meter bounds spend. A distinct `mcp:rules.ai:call` remains a clean follow-up (the scope's
  proposal) — deferred, not rejected.
- **`ai.embed`** — left erroring ("not supported"), per the scope; wire when an embedding provider
  surface exists.

## Docs

- Skill extended: `docs/skills/rules/SKILL.md` — a new "`ai.*` runs against the workspace's SELECTED
  model" subsection (grounded run + the honest "AI configured?" note) + two gotchas + Related links.
- This session log; scope promotes to `docs/public/rules/rules.md` when the public doc is next revised.
