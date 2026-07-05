# A workspace's sealed model key never reached its pick of a built-in agent

**Area:** agent · **Status:** resolved · **Date:** 2026-07-05
**Scope:** `docs/scope/agent/active-agent-wiring-scope.md` (Slice 2 — per-workspace model resolution)
**Session:** `docs/sessions/agent/config-endpoint-overlay-session.md`

## Symptom

The In-house agent card showed **"model key: set ✓"** yet still answered with the honest
placeholder:

```
no in-house model is configured on this node; select an external runtime
(e.g. open-interpreter-default) or wire a model provider
```

The workspace had picked the seeded built-in `builtin.in-house-glm-4.6`, then used the catalog's
"Set model key" to seal a Z.AI token. The key was genuinely stored — the UI truthfully reported
"set ✓" — but a run resolved to no key and the provider call could not authenticate.

## Root cause

The catalog's "Set model key" for the **active pick** (`ActiveModelKey.tsx` → `useAgentCatalog.ts`
`setActiveKey`) seals the token and writes **only the resulting secret PATH onto
`agent.config.model_endpoint.api_key_secret`** — never onto the definition. That is deliberate: a
built-in definition is read-only (`is_builtin` rejects update), so the workspace's *selection* of it
carries the key, not the shared built-in record.

But the read side was never finished. `resolve_workspace_model` resolved the key from the
**definition's** endpoint:

```rust
let ep = &def.model_endpoint;              // the DEFINITION (agents.toml), read-only
let secret = ep.api_key_secret.as_deref(); // None for a built-in
```

`resolve_active_definition` returns the built-in record verbatim from `agents.toml`, whose
`api_key_secret` is `None`. The config's sealed PATH was **never overlaid** onto the endpoint the
resolver read. So `secret = None` → `resolve_endpoint_key_host` fell back to the `ZAI_API_KEY` node
env (also unset) → the adapter was built with an empty key → an unauthenticated call. The whole
"a workspace keys its pick of a built-in" feature was wired end-to-end on the **write** side and
dead on the **read** side.

## Fix

Add a config→definition endpoint overlay and apply it in the resolver before building the key/model.

`crates/host/src/agent/overlay_endpoint.rs` (new) — `overlay_config_endpoint(def, cfg)` merges the
workspace's `agent.config.model_endpoint` (a nullable `ModelEndpointPatch`) over the definition's
`DefinitionEndpoint`: a present config field wins, an absent one inherits the preset. Required
fields (`provider`/`model`) only override when present **and non-empty**, so an empty config value
never blanks out a built-in's provider.

`crates/host/src/agent/resolve_model.rs` — after `resolve_active_definition`, read the config and
overlay it:

```rust
let cfg = get_agent_config(&node.store, ws).await.ok().flatten();
let ep = overlay_config_endpoint(
    &def.model_endpoint,
    cfg.as_ref().and_then(|c| c.model_endpoint.as_ref()),
);
```

The endpoint hash (cache key) is computed from the overlaid endpoint, so a key rotation on
`agent.config` (which already `invalidate_workspace_model`s) rebuilds correctly.

This is exactly the "the active `agent.config` is workspace-scoped and can own a sealed secret path"
contract the scope doc and both UI comments describe — it was just never implemented on the read
path.

## Regression test

`crates/host/tests/agent_active_model_test.rs::a_builtin_pick_resolves_its_sealed_key_from_agent_config`
— seeds the built-in catalog, seals a `Workspace` secret, points `active_definition` at
`builtin.in-house-glm-4.6` with the sealed PATH on `agent.config` **only** (the built-in record
carries no secret, asserted explicitly), then resolves the model through a key-recording builder and
asserts the **sealed value reached the adapter**. Fails-before (builder saw `None`) / passes-after.
Plus four `overlay_endpoint` unit tests (none-config passthrough, sealed-key overlay onto a built-in,
present-wins/absent-inherits, empty-required-falls-back).

## Lesson

A write path that stores config on one record (`agent.config`) and a read path that resolves from a
different record (the definition) will silently diverge — "set ✓" in the UI is only honest if the
resolver reads the same layer the writer wrote. When a selection layer is *designed* to override a
read-only preset, the overlay must exist on the **read** side too, not just in the write's intent.
