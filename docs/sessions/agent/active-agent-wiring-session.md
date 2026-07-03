# Session — active-agent wiring (the active pick is the one implicit agent everywhere)

Status: in-progress
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

(filled as slices land)
