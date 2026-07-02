# Flows scope — the Node-RED deploy model (dirty-tracked Deploy, Enable/Disable, live-values toggle)

Status: scope (the ask). Promotes to `public/flows/` once shipped.

The runtime already **converges on save**: every firing re-reads the flow fresh
(`run.rs` loads it at run start), and the reactor tick re-reads the durable flow set
every few seconds and re-arms each source with its *current* config
(`reconcile_flows` → `arm_source`). So a `flows.save` **is** the moment an edit goes
live — the machinery is already Node-RED-shaped. What was missing was the **operator
model on top of it**: the canvas gave no signal that edits were unsaved, "Deploy" in
the banner actually meant *enable*, and there was no way to turn live-value painting
off. This scope puts the Node-RED posture on the existing engine:

- **The canvas is a draft.** Edits (node add/delete, wiring, config) change component
  state only. **Nothing reaches the running system until Deploy.**
- **Deploy = `flows.save`** — one atomic commit of the whole graph. The button is
  **lit only when the canvas differs from the saved flow** (dirty), disabled otherwise.
- **Enable / Disable = `flows.enable`** — "should this flow ever fire" (durable).
  Disable means it never runs again until re-enabled. (This is today's banner toggle,
  **renamed** from the misleading "Deploy/Stop".)
- **Start / Stop = `flows.run` / `flows.cancel`** — the manual run + mid-run cancel,
  **unchanged** (the operator is happy with these).
- **Live values on/off** — a toggle that gates the SSE watch + the node_state/runs
  polling. Off by default is cheap; on paints each wire's current value (Node-RED debug).

## Goals

- A **dirty flag** derived by comparing the canvas buffer (nodes + edges + per-node
  configs) against the last-saved flow. Deploy is enabled ⇔ dirty; after a successful
  Deploy the flag clears (the saved snapshot advances).
- **Deploy is the only path that mutates the running graph.** Per-node quick-apply
  (`flows.node.update`) stays as a fast path for tuning one node's config, and it too
  clears that node's dirtiness.
- **Enable/Disable** promoted to a first-class toolbar control with unambiguous copy.
- **Live-values toggle** gating the observe cost; default off, remembered per session.
- **Backend: no orphaned live sockets.** Deleting a flow (or removing a source node in
  an edit) must **disarm** that source — today the reconciler only disarms sources on a
  flow *still in the list*, so a tombstoned flow's socket leaks forever.

## Non-goals

- **Immediate source restart on Deploy.** v1 lets the reactor tick re-arm changed
  sources (a few seconds — invisible for a schedule). A "Deploy kicks a reconcile now"
  optimization is a noted follow-up, not this slice.
- **Per-node deploy scoping** (Node-RED's "modified nodes only" deploy mode). Deploy is
  whole-graph here; the engine already only re-arms what changed on the next tick.
- **Client-durable drafts.** The dirty buffer is transient component state (matching the
  canvas scope's existing "transient unsaved buffer" allowance). A page reload reloads
  the saved flow — no local draft persistence.
- New run-engine behavior — this is the operator model over the shipped runtime.

## Intent / approach

**A UX layer, plus one small backend correctness fix.**

*Frontend.* A pure `flowDirty(saved, buffer)` comparator (its own file) drives a
`deployable` boolean. A new **`FlowToolbar`** component owns the primary controls
(Deploy/Run/Stop/Enable/live-values) so `FlowCanvas` drops back under the 400-line file
limit (it is 593 today — rule 8). The banner's enable toggle is **removed** (its job
moves to the toolbar's Enable/Disable); the banner stays purely informational (armed,
next fire, count). The live-values toggle gates the existing `nodeState`/`runs` polling
and the SSE `watch` — when off, the canvas shows the last-loaded snapshot statically and
opens no stream.

*Backend.* One added reconcile step: after the per-flow arm/disarm pass, scan
`flow_node_state` for `armed:true` markers whose owning flow is **absent or tombstoned**
(or whose node no longer exists in the flow) and **disarm** each. This closes the leaked
socket on delete + on source-node removal. Idempotent, workspace-scoped, logged.

Rejected alternative (frontend): keep auto-applying every edit and just add a "saved"
toast. It fails the control-system posture — an operator wiring a half-finished graph
that drives hardware must not have partial edits go live keystroke-by-keystroke. The
deploy-gate is the safety property, not decoration.

Rejected alternative (backend): teardown-on-delete inside `flows_delete` (disarm there
before tombstoning). It's tempting but wrong for two reasons: `flows_delete` is a
store-surface verb with no node handle, and a **source-node removal via an edit** (not a
delete) has the identical leak — the reconcile-sweep covers both with one mechanism and
is self-healing after a crash mid-delete. Decision 13's "converge to released" already
lives in the reconciler; this extends it to orphans.

## How it fits the core

- **Tenancy / isolation:** the dirty compare is client-local. The disarm-sweep reads
  `flow_node_state` and the flow set **workspace-scoped** (the reconciler already runs
  per-ws under the node's routed principal); it can never disarm another ws's source.
- **Capabilities:** no new verb, no new cap. Deploy is `flows.save` (existing
  `mcp:flows.save:call`), Enable is `flows.enable`, live-values reads
  `flows.node_state`/`flows.runs.get` + the `flows.watch` SSE — all already gated. The
  disarm-sweep runs under the reactor's system principal (unchanged authority).
- **Placement:** the sweep runs in the same `reconcile_flows` pass — `either`, owner-
  elected, no `if cloud`.
- **State vs motion:** dirtiness is a view-only projection of state (saved record vs
  buffer); the disarm-sweep converges durable state → releases motion (the socket). No
  new store of record.
- **Stateless extensions (rule 4):** disarming an orphan calls the ext's `disarm` tool —
  the socket lived in the (supervised) ext, never in a flow instance. Unchanged.

## Example flow

1. Author drags a node + wires it → **Deploy lights up** (canvas ≠ saved). Nothing has
   fired yet; the running flow is still the last-deployed graph.
2. Click **Deploy** → `flows.save` → new version; Deploy greys out (buffer == saved).
   The next reactor tick re-arms any changed source with the new config.
3. Author tweaks one node's `setpoint` in the panel → **Save node** (`flows.node.update`)
   → that node's dirtiness clears without a whole-graph Deploy.
4. **Disable** → `flows.enable {enabled:false}` → armed banner shows "Disabled — nothing
   fires"; the reactor disarms its sources next tick. **Enable** re-arms.
5. **Live values: on** → the canvas opens the watch/poll and paints each wire's current
   value; **off** → it stops polling and the values freeze at the last snapshot.
6. **Delete** the flow → tombstoned; the next reconcile pass finds its `armed:true`
   node_state orphans and **disarms** them (no leaked MQTT socket).

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), real `mem://` + real bus +
real caps — no mocks:

- **Capability deny / workspace isolation:** unchanged verbs already covered; the
  disarm-sweep is asserted to only touch the caller-ws's node_state (a ws-B armed source
  survives a ws-A reconcile).
- **Orphan disarm on delete (regression):** arm a source flow, `flows.delete` it, run a
  reconcile pass → assert `disarm` was called and the `armed` marker cleared. A second
  pass is a no-op (idempotent).
- **Orphan disarm on source-node removal:** save a flow removing an armed source node →
  reconcile → that node disarmed, the flow's remaining sources untouched.
- **Frontend (Vitest, real spawned gateway):** Deploy is **disabled on a pristine open**,
  **enabled after an edit**, and **disabled again after a successful Deploy**; Enable/
  Disable flips the durable flag + banner; live-values off opens no stream (no poll) and
  on renders values; per-node Save clears only that node's dirtiness.
- **Frontend unit:** `flowDirty` — identical graphs → clean; a config/edge/node
  add/delete → dirty; config key-order-insensitive.

## Risks & hard problems

- **Dirty false-positives** from serialization order (config key order, edge order). The
  comparator must normalize (sort needs, stable-stringify configs) so re-opening a saved
  flow reads clean. Covered by the unit test.
- **The disarm-sweep must not disarm a live source mid-edit.** It only targets orphans
  (flow gone/tombstoned OR node absent from the current flow) — a flow still present with
  the node present is converged by the existing arm branch, never swept. Assert both.
- **Live-values default off** could confuse ("why no values?"). Mitigation: the toggle is
  labeled and defaults visible; the armed banner still shows the run count so "it's
  firing" is legible without values on.

## Open questions — RESOLVED (this session)

1. Does Deploy restart sources immediately or on the tick? **On the tick** for v1
   (non-goal to kick a reconcile inline). Revisit if the few-second lag is felt.
2. Live-values default on or off? **Off** — the observe cost is opt-in (Node-RED debug
   is opt-in per node); the banner covers "is it firing".
3. Keep per-node `flows.node.update` alongside Deploy? **Yes** — it's the fast path for
   tuning one config and Node-RED has no equivalent; it clears only that node's dirtiness.

## Related

- README `§3` (state vs motion, capability-first, the wall), `§6.10` (jobs).
- Sibling scopes: `flow-runtime-control-scope.md` (the async drive + `flows.node.update`
  + `flows.watch` this builds the operator model over), `triggers-lifecycle-scope.md`
  (the reconciler + arm/disarm this extends with the orphan sweep), `flows-canvas-scope.md`
  (the canvas + transient-buffer allowance), `armedState.ts` (the banner truth).
- Promotes to `public/flows/flow-deploy-ux.md`.
