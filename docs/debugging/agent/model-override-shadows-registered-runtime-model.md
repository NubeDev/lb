# The per-run model override shadowed the runtime's registered model

**Area:** agent · **Status:** resolved · **Date:** 2026-07-03
**Scope:** `docs/scope/agent/active-agent-wiring-scope.md` (Slice 2 — per-workspace model resolution)
**Session:** `docs/sessions/agent/active-agent-wiring-session.md`

## Symptom

`crates/host/tests/agent_routed_test.rs::an_edge_invokes_the_hub_agent_over_the_routed_namespace`
failed after the active-agent-wiring diff landed:

```
assertion `left == right` failed: the hub's agent ran the loop and replied
  left: "no in-house model is configured on this node; select an external runtime
         (e.g. open-interpreter-default) or wire a model provider"
 right: "routed: done"
```

The routed edge→hub invoke reached the hub's agent, but instead of driving the **mock model**
the test installed (`RuntimeRegistry::with_default(mock)` handed to `serve_agent`), the loop ran
the honest **UnconfiguredModel** placeholder and returned "no in-house model is configured".
The test file is NOT in the scope diff — the change to `dispatch.rs` broke it.

## Root cause

Slice 2 added a per-run model override so an implicit run drives the workspace's *picked* model
(`RunContext.model_override`, resolved by `resolve_workspace_model` at run start). `dispatch.rs`
set it **unconditionally** for the default runtime:

```rust
let model_override = if runtime.id() == DEFAULT_RUNTIME {
    Some(super::resolve_workspace_model(node, caller, ws).await)  // always Some(...)
} else { None };
```

But `resolve_workspace_model` **never returns `None`** — with no active pick and no configured
node model it returns the honest `UnconfiguredModel` placeholder (its fallback tier reads
`node.runtimes().default_model()`). And `InHouseRuntime::run` *prefers* `model_override` over the
model it was registered with:

```rust
let model = ModelHandle(ctx.model_override.clone().unwrap_or_else(|| self.model.clone()));
```

So for a workspace with no pick, the override forced `UnconfiguredModel`, **shadowing the model
the runtime was actually registered with**.

The trap is that `resolve_workspace_model`'s node-fallback reads `node.runtimes()` — the *node's*
registry — but the registry a run is **dispatched through** is the one passed into
`invoke_via_runtime`, which is **not always** `node.runtimes()`. In production `serve_agent` is
handed `node.runtimes()` (they coincide, so the scope's own `agent_active_model_test` — which uses
`&node.runtimes()` and configures a real pick — stayed green). But the routed test legitimately
boots a bare hub and passes a **separate** `RuntimeRegistry::with_default(mock)` to `serve_agent`;
the node's own registry is `UnconfiguredModel`. The override read the wrong registry.

## Fix

`crates/host/src/agent/dispatch.rs` — only override with a **configured** model; an unconfigured
resolution yields `None`, so `InHouseRuntime` falls back to *its registered model*:

```rust
let model_override = if runtime.id() == DEFAULT_RUNTIME {
    let resolved = super::resolve_workspace_model(node, caller, ws).await;
    resolved.is_configured().then_some(resolved)
} else { None };
```

This preserves the intended ladder exactly: **active workspace pick → the registered runtime
model → (that model's own) unconfigured answer** — and stops the override from shadowing the
served registry when the workspace has no pick. Rules are unaffected: `resolve_rule_model` still
wants the `UnconfiguredModel` for its honest "AI not configured for rules" error and calls
`resolve_workspace_model` directly (not through this seam).

## Regression test

`agent_routed_test.rs::an_edge_invokes_the_hub_agent_over_the_routed_namespace` is the guard —
verified fails-before (panic at `:120`, the message above) / passes-after. It exercises the exact
divergence: a served registry that is NOT the node's default, a workspace with no pick, an
implicit (`runtime` omitted → default) run that must ride the *registered* model. The scope's
`agent_active_model_test::the_in_house_loop_drives_the_picked_workspace_model` remains green
(a *configured* pick → override IS set → picked model answers), so both directions of the ladder
are pinned.

## Lesson

A per-run override that "always wins" must only win when it carries a **real** value — a resolver
that never returns `None` (returns a placeholder instead) will silently shadow a lower tier. And a
fallback that reads global node state (`node.runtimes()`) diverges from a run dispatched through a
*passed-in* registry: the honest-unconfigured placeholder is not a substitute for "no override —
use what this runtime was registered with".
