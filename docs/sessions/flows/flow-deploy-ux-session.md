# Session — Node-RED deploy model for flows (dirty Deploy + Enable/Disable + live-values toggle)

Date: 2026-07-02 · Branch: `ce-node-wiring` · Scope: [`flow-deploy-ux-scope.md`](../../scope/flows/flow-deploy-ux-scope.md)

## The ask

Make the flow canvas usable like Node-RED: the canvas is a **draft**, and edits only reach
the running system on **Deploy** — a button that lights up **only when something changed**.
Keep Start/Stop (manual run) as-is; add Enable/Disable (durable "never run again"); add a
**live-values on/off** toggle. Reported pain: "start but no clear deploy", "Deploy meant the
wrong thing", live values always on.

## What shipped

**Frontend — the operator model (all under `ui/src/features/flows/`):**

- `flowDirty.ts` — pure comparator: canvas buffer vs the deployed graph, normalized so key/
  needs/node order and version/lifecycle flags never read as dirty. Drives Deploy's enabled
  state. Unit-tested (`flowDirty.test.ts`, 9 cases).
- `FlowToolbar.tsx` — the primary controls: **Deploy** (enabled ⇔ dirty; `flows.save`), Run/
  Stop (`flows.run`/`flows.cancel`, unchanged), Suspend/Resume (mid-run), **Enable/Disable**
  (`flows.enable`, renamed from the misleading banner "Deploy/Stop"), and a **Live values**
  switch. On a **scheduled** flow (cron/source trigger) the Run button reads **"Test run"** —
  a one-off manual fire — because the real 24/7 firing is Enable's job (its trigger fires
  headless); on a manual-only flow it stays "Run" (the only way it ever runs). Unit-tested
  (`FlowToolbar.test.tsx`, 7 cases).
- `useLiveValues.ts` — owns the persistent node-state + the live-values toggle; OFF fetches
  nothing and runs no poll (the observe cost is opt-in). Default OFF.
- `flowTransfer.ts` — the import/export JSON round-trip, extracted from the canvas.
- `FlowCanvasHeader.tsx` — the header bar (toolbar + undo/export/import/delete + status).
- `FlowCanvas.tsx` — rewired: tracks `deployedFlow` (advances on Deploy + per-node Save so
  dirty clears), gates the SSE watch + polling behind `liveValues`, delegates the header.
  **593 → 496 lines** (the concerns are now in 5 focused files; the remainder is core canvas
  graph state + handlers).
- `FlowArmedBanner.tsx` — now purely informational (its Enable/Disable toggle moved to the
  toolbar); test updated.

**Backend — the leaked-socket fix (`rust/crates/host/src/flows/`):**

- `orphan_sweep.rs` — `sweep_orphan_sources`: after the per-flow arm/disarm pass, scan the
  armed markers and **disarm any orphan** (its flow deleted/tombstoned, or its source node
  removed by an edit). Wired into `reconcile_flows` (new `ReconcilePass.orphans_disarmed`).
  Idempotent, workspace-scoped, self-healing after a crash mid-delete.
- Fixed a real latent bug found while building this: `arm_source` wrote the armed marker with
  **no `_type`**, so `disarm_source` could never resolve the ext's `disarm` tool — the socket
  release was a silent no-op. Now `arm_source` + `reconcile_flows` persist `_type` on the
  marker. See [`debugging/flows/disarm-cant-resolve-ext-tool-arm-omitted-type.md`](../../debugging/flows/disarm-cant-resolve-ext-tool-arm-omitted-type.md).

## Why this shape

The runtime already **converges on save** (every firing re-reads the flow; the reactor re-arms
sources with current config each tick). So "Deploy = `flows.save`" is honest — it genuinely is
the moment edits go live. The work was the **operator model on top**, not a new engine. Per-node
`flows.node.update` stays as a fast single-node apply (Node-RED has no equivalent) and clears
just that node's dirtiness. Live-values default OFF because the armed banner already shows "it's
firing" and the poll/SSE is a real cost.

Rejected: auto-apply every edit + a "saved" toast — unsafe for a control system driving hardware
(half-finished edits would go live keystroke-by-keystroke). The deploy-gate is the safety
property. Rejected: teardown inside `flows_delete` — a source-node *removal* leaks identically,
and the reconcile-sweep covers both with one self-healing mechanism.

## Tests (green)

- Rust: `flows_orphan_sweep_test` (3), plus `flows_ext`/`flows_triggers`/`flows_runtime_control`/
  `flows_multi_trigger` (39) — **42 passed**, confirming the `_type` change didn't regress arm/
  disarm/reconcile. (`cargo fmt` clean.)
- UI unit: `flowDirty` (9), `FlowToolbar` (6), `FlowArmedBanner` (4), plus the existing flows
  unit suite — **48 passed**. eslint clean on all changed files.
- Pre-existing, unrelated failures: `github_bridge_normalize_test` needs a prebuilt
  `github_bridge_ext.wasm` artifact absent in this env (not touched by this work). The repo-wide
  `tsc` `LucideIcon`-as-JSX errors are the known `@types/react` mismatch, not from these files.

## Follow-ups (noted, not done)

- Deploy could kick a reconcile inline for true Node-RED "restart changed nodes now" instead of
  waiting a tick (v1 lets the tick handle it — invisible for a schedule).
- Live-values preference could persist per session (currently resets on flow open).

## Related

- Scope: [`flow-deploy-ux-scope.md`](../../scope/flows/flow-deploy-ux-scope.md).
- Builds on: `flow-runtime-control-scope` (async drive, `flows.node.update`, `flows.watch`),
  `triggers-lifecycle-scope` (the reconciler + arm/disarm), `flows-canvas-scope` (the canvas).
- Debugging: [`disarm-cant-resolve-ext-tool-arm-omitted-type.md`](../../debugging/flows/disarm-cant-resolve-ext-tool-arm-omitted-type.md).
