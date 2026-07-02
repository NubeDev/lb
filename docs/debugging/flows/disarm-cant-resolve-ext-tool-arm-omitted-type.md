# flows — `disarm_source` could never call the ext's `disarm` tool (arm omitted `_type`)

**Status:** resolved · **Date:** 2026-07-02 · **Area:** flows

## Symptom

Disabling or deleting a source flow left the extension's live socket open. `disarm_source`
ran, cleared the `armed` marker, and returned `Ok(())` — so nothing *looked* wrong — but it
**never called the extension's `<ext>.disarm` tool**. The socket in the (native) sidecar
stayed subscribed. Surfaced while building the orphan-source sweep (flow-deploy-ux-scope): a
test that armed then disarmed a source and expected the `disarm` tool to fire found it was
never invoked.

## Root cause

The disarm path resolves which ext tool to call from the node's **`_type`**, read back from
the `flow_node_state:{flow}:{node}` marker:

```rust
// disarm_source: read the marker, recover `_type`, resolve `<ext>.disarm`
let node_type = read(store, ws, FLOW_NODE_STATE_TABLE, &format!("{flow}:{node}"))
    .and_then(|v| v.get("_type") …)   // ← expects `_type` on the marker
    .unwrap_or_default();
if let Some(tool) = resolve_disarm_tool(store, ws, &node_type).await { … }
```

But `arm_source` wrote the marker as `{ armed: true, series }` — **no `_type`**. So the
read-back was always empty, `resolve_disarm_tool("")` returned `None`, and the `disarm` call
was skipped. The bug was invisible because disarm still cleared the marker and returned `Ok`;
only asserting the tool-call (or observing the leaked socket) reveals it. It also can't be
fixed by reading the type from the flow at disarm time — an **orphan** disarm (the flow was
deleted) has no flow to read.

## Fix

Persist `_type` on the armed marker so disarm (including orphan disarm, after the flow is
gone) can resolve the ext tool from the durable record:

- `arm_source` now writes `{ armed: true, series, _type }` (`source.rs`).
- `reconcile_flows` stamps `n.node_type` into the config it passes to `arm_source`, so the
  reconciler-armed path carries the type too (`reconcile.rs`).

The new orphan sweep (`orphan_sweep.rs`) relies on this: it disarms markers whose flow/node
no longer exists, and needs the marker's own `_type` to reach the ext's `disarm`.

## Regression

`flows_orphan_sweep_test.rs`:
- `delete_orphans_the_source_and_the_sweep_disarms_it` — arm → delete → reconcile disarms the
  orphan (and is idempotent on a second pass).
- `source_node_removal_orphans_only_the_removed_node` — a removed source node is disarmed; the
  kept node stays armed.
- `ws_a_sweep_leaves_ws_b_armed_source_untouched` — workspace isolation.

## Lesson

A teardown that resolves its target from a durable marker must have the arm path **write
everything the teardown needs** onto that marker — the flow record may be gone by teardown
time. A disarm that returns `Ok` without calling the ext tool is a silent no-op; assert the
tool-call, not just the cleared flag.
