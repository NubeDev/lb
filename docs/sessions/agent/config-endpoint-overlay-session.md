# Session — config→definition endpoint overlay (In-house agent "model key: set ✓" but unconfigured)

**Date:** 2026-07-05 · **Area:** agent · **Scope:** `docs/scope/agent/active-agent-wiring-scope.md` (Slice 2)

## The ask

"Why can't I use the In-house agent? Something hasn't been finished." The catalog card showed
**"model key: set ✓"** yet the agent answered the honest unconfigured placeholder.

## Diagnosis

Two paths make the In-house agent work (`resolve_workspace_model`, precedence):

1. the workspace's active definition's endpoint → a real adapter (the per-ws pick), else
2. the node-level in-house `default_model` (the `LB_AGENT_MODEL_*` env tier), else
3. the honest `UnconfiguredModel` placeholder.

The adapter code is **finished** — `node/src/agent.rs::adapter_for` already builds a real
`AiGateway<OpenAiCompat>` for `zaicoding`/`openai`/`openai-compat` (a stale comment in that file
still says "no real provider adapter exists" — untrue since the swap landed).

**Option A** (node env) simply wasn't configured — no `LB_AGENT_MODEL_*` in `make dev`/docker.
The user chose **Option B** (the per-workspace "model key: set ✓" path), which was a genuine
unfinished seam.

## The bug (Option B)

"Set model key" on the active pick (`ActiveModelKey.tsx` → `useAgentCatalog.ts::setActiveKey`)
seals the token and writes **only the resulting PATH onto `agent.config.model_endpoint.api_key_secret`**
— deliberately, since a built-in definition is read-only. But `resolve_workspace_model` read the key
from the **definition's** endpoint (the built-in `agents.toml` record, `api_key_secret = None`) and
never overlaid the config's selection layer. So the sealed key was invisible at run time → env
fallback (unset) → an unauthenticated adapter. Full write/read table in the debugging entry.

## The fix

- **New** `crates/host/src/agent/overlay_endpoint.rs` — `overlay_config_endpoint(def, cfg)`: merge the
  workspace's `agent.config.model_endpoint` (nullable `ModelEndpointPatch`) over the definition's
  `DefinitionEndpoint`. Present config field wins; absent inherits the preset. Required fields
  (`provider`/`model`) override only when present **and non-empty** (never blank out a built-in).
- **Edited** `crates/host/src/agent/resolve_model.rs` — read the config after `resolve_active_definition`
  and overlay it before resolving the key and building the model. The cache key hashes the overlaid
  endpoint; `agent.config.set` already invalidates the ws entry, so a key rotation rebuilds.
- **Registered** the module in `agent/mod.rs`.

Layering held: no new host→role dependency; the builder seam is unchanged.

## Tests (all green)

- `overlay_endpoint` unit tests (4): none-config passthrough, sealed-key overlay onto a built-in,
  present-wins/absent-inherits, empty-required-falls-back.
- `agent_active_model_test::a_builtin_pick_resolves_its_sealed_key_from_agent_config` (new, e2e):
  seeds the built-in catalog, seals a `Workspace` secret, points `active_definition` at
  `builtin.in-house-glm-4.6` with the sealed PATH on `agent.config` only (built-in record has no
  secret, asserted), resolves through a key-recording builder, asserts the **sealed value reached the
  adapter**. Fails-before / passes-after.
- Re-ran `agent_active_model_test` (7), `agent_config_test` (6), `agent_persona_catalog_test` (5),
  `agent_default_runtime_test` (8), `rules_ai_wiring_test` (8), and the host lib unit suite (119) —
  all green. `cargo fmt` clean.

## Follow-ups (noted, not done here)

- The stale "no real provider adapter exists today" comment block in `node/src/agent.rs` (lines
  ~76–86) should be updated — the adapter it describes as deferred has shipped.
- **Option A** for a node-wide in-house default: wire `LB_AGENT_MODEL_*` into `make dev` (guarded
  like the existing `ZAI_API_KEY` warning) if a keyless-per-workspace default is wanted. Not needed
  now that Option B resolves.
