# Flows scope — editor UI polish ("less is more")

Status: scope (the ask). Promotes to `public/flows/` once shipped.

The flows editor works but doesn't *feel* like Node-RED: the header shows ~10 controls at
once, the node config panel is a raw unstyled form, the config panel and the debug drawer
fight for the right edge as two separate panels, and Export is a blind file download. This
scope is a **UI-only** consolidation pass: fewer things visible at any moment, one right-side
dock, a designed config form, and a proper export/import dialog — **every existing feature
keeps working**; nothing is removed from the surface, only from the default view.

## Goals

- The idle header shows **at most 4 primary controls**; everything else is reachable in one
  click but not permanently on screen.
- Config and Debug share **one right dock with tabs** — they never render side by side.
- The node config panel looks like a designed product form, not a bare schema dump.
- Export/Import becomes a dialog with preview, copy, and pretty/compact — Node-RED parity.
- The canvas gains the interaction polish (selection, hover, edge, motion states) that makes
  it feel smooth, within the product register (motion conveys state, 150–250 ms, reduced-motion
  respected).

## Non-goals

- **No verb changes.** `flows.save/run/cancel/suspend/resume/enable/patch_run/node.update`
  and the debug SSE feed are untouched. This is presentation + composition only.
- No new nodes, no descriptor changes, no runtime behavior changes.
- No palette/left-rail redesign (fine as is; separate ask if wanted).
- No theming/branding work beyond using the existing tokens correctly.

## Intent / approach

Four slices, each independently shippable. The alternative — a ground-up canvas rewrite or
swapping React Flow for something else — is rejected: the graph layer is fine; the problem is
chrome density and unstyled panels, which is a composition fix, not an engine fix.

### 1. Toolbar consolidation (`FlowToolbar.tsx`, `FlowCanvasHeader.tsx`)

Today the header can show: Deploy, Run, Suspend, Resume, Stop, Enable/Disable, Live values,
Undo, Export, Import, Debug, status badge, Delete — all at once. Restructure into
**primary / contextual / overflow**:

- **Primary (always visible):** `Deploy` (unchanged semantics — the one write-to-runtime
  button, disabled when clean), a **single morphing Run ⇄ Stop button** (idle → `Run`;
  run active → destructive `Stop`), and the Debug dock toggle.
- **Contextual (only mid-run):** one **Pause ⇄ Resume toggle** replaces the separate
  Suspend + Resume buttons — the pair is mutually exclusive by definition (a run is either
  suspended or not), so showing both is pure noise. Rendered next to Stop while a run is in
  flight, gone otherwise.
- **Overflow (`⋯` dropdown menu):** Enable/Disable, Live values, Undo, Export…, Import…,
  Delete flow (destructive, separated). These are occasional operator actions, not
  every-minute controls. Undo additionally keeps `mod+z` so moving it costs nothing.
- The run-status badge and error text stay in the header right side (they're feedback, not
  controls). The `animate-pulse` on dirty-Deploy is replaced with a static accent treatment —
  a permanently pulsing primary button is decorative motion, which the product register bans.

Idle header: **Deploy · Run · Debug · ⋯** (+ status). Running: **Deploy · Stop · Pause ·
Debug · ⋯**. That is the whole ask: same features, a third of the chrome.

### 2. One right dock (`RightDock.tsx`, new)

`FlowCanvas.tsx:483-499` currently renders `NodeConfigPanel` and `DebugPanel` as two sibling
right panels — select a node with debug open and both stack up. Replace with a single
`RightDock` owning the right edge:

- Tabs: **Config | Debug**. Selecting a node opens/switches to Config; the Debug button
  opens/switches to Debug. Both "open" states collapse into one dock with one close button.
- The dock is resizable (reuse `panel-builder/wizard/useSplitPane.ts` if it generalises;
  otherwise a small `useDockResize` hook) and persists width + last tab per the existing
  prefs pattern.
- `NodeConfigPanel` and `DebugPanel` stay as separate one-responsibility files; `RightDock`
  is only the tab shell + layout (FILE-LAYOUT).

### 3. Node config redesign (`NodeConfigPanel.tsx`, `SchemaForm.tsx`)

- Header: node id as the title, descriptor title + type as a muted subtitle line with the
  node-kind icon — not the current `a (Count (input size))` concatenation.
- The two stacked run-state banners collapse to **one compact status line** (locked /
  patchable), with the long explanation moved to a tooltip/`title`.
- `SchemaForm` visual pass: consistent label + control + help-text rhythm from the schema's
  `description`, grouped sections when the schema nests, proper empty state ("This node has
  no configuration") instead of bare text, inline per-field errors instead of one bottom line.
- **One primary action, context-aware**, in a sticky footer: normally `Save node` (the common
  case); during an active run on an unexecuted node, `Patch run` becomes primary and
  `Save node` secondary. `Save flow` moves into the footer's small overflow — it duplicates
  the header's Deploy-adjacent mental model and is the rarest of the three. All three
  behaviors remain.

### 4. Export/Import dialog (`FlowTransferDialog.tsx`, new; `flowTransfer.ts` unchanged)

Replace the blind download / hidden file input with one dialog (Node-RED's export UX):

- **Export:** JSON preview (scrollable, monospace), pretty ⇄ compact toggle, **Copy to
  clipboard** and **Download** actions, and scope choice **whole flow vs. selected nodes**
  when a selection exists.
- **Import:** same dialog, second tab — paste JSON *or* pick a file; parse + validate via the
  existing `parseImportedFlow`, show the node/edge count as confirmation before applying.
- A modal is justified here (the product register's "exhaust inline first" rule): the action
  is deliberate, bounded, and needs a preview surface the header can't host.

### 5. Canvas feel (small, bounded)

- Node hover/selected states (border + subtle shadow via tokens), edge hover highlight,
  smoother default edge curvature.
- Live-value dots animate only on value change (state feedback), 150–250 ms ease-out, with a
  `prefers-reduced-motion` fallback to instant swap.
- No page-load choreography, no decorative motion (product register).

## How it fits the core

- **Tenancy / capabilities / placement / MCP / data / bus / sync / secrets:** N/A — this
  scope changes no verbs, records, subjects, or grants; it recomposes existing UI over the
  shipped `flows.*` surface. The capability-deny and isolation behavior of every action is
  already owned by the verbs it calls.
- **Core knows no extension (rule 10):** holds — the dock/toolbar treat nodes and descriptors
  as opaque data from `flows.nodes`; nothing branches on a node or extension id.
- **One responsibility per file:** `RightDock.tsx`, `FlowTransferDialog.tsx`, and any resize
  hook are new files; `FlowToolbar`/`FlowCanvasHeader`/`NodeConfigPanel` shrink, not grow.

## Example flow

1. Operator opens `chain4`. Header shows **Deploy (disabled/clean) · Run · Debug · ⋯**.
2. Clicks Run. Header becomes **Deploy · Stop · Pause · Debug · ⋯** with the running badge.
3. Clicks a node → the right dock opens on **Config**; clicks Debug → same dock switches to
   the **Debug** tab; back to Config via the tab, one close button collapses the dock.
4. On the unexecuted node, the footer's primary action reads **Patch run**; after the run
   completes it reads **Save node**.
5. `⋯ → Export…` opens the dialog; operator toggles compact, copies the JSON, closes.

## Testing plan

Per `scope/testing/testing-scope.md` — no fakes; the gateway tests run against the real node:

- **Unit (vitest):** `FlowToolbar.test.tsx` rewritten for the morphing Run/Stop and
  Pause⇄Resume states; `FlowCanvasHeader.test.tsx` for the overflow menu (every relocated
  action still fires its callback); new `RightDock.test.tsx` (tab switching, node-select
  opens Config, single-instance invariant — Config and Debug never co-render); new
  `FlowTransferDialog.test.tsx` (pretty/compact, copy, selected-nodes scope, paste-import
  validation path).
- **Gateway (real node):** existing `FlowsCanvas.gateway.test.ts`, `flowsDebug.gateway.test.ts`,
  `FlowsRuntimeControl.gateway.test.ts` must stay green unmodified in behavior — they prove
  every verb is still reachable from the recomposed UI. Extend the runtime-control test to
  drive suspend→resume through the single toggle.
- **Capability-deny / isolation:** already covered by the untouched verb layer's tests; the
  UI adds no new call sites, so no new deny cases — state this in the session doc rather than
  duplicating tests.

## Risks & hard problems

- **The overflow menu hiding too much.** Enable/Disable is safety-relevant; the flow's
  disabled state must stay visible even though the *control* moves — keep a "Disabled" badge
  in the header when `enabled === false`.
- **Run/Stop morphing race:** the button flips on `runActive`, which lags the click by a
  round-trip; debounce/disable during transition or a double-click can cancel the run just
  started.
- **Dock state vs. executed-node-lock:** switching tabs must not drop the config edit buffer
  mid-run; the buffer stays owned by the canvas (as today), the dock is stateless chrome.
- **`useSplitPane` reuse** may not generalise cleanly from the wizard; budget for a small
  dedicated hook rather than forcing it.

## Open questions

1. ~~Does `Save flow` survive in the config footer?~~ **Resolved in the build session:** dropped
   — header `Deploy` is the only whole-flow write; the footer keeps Save node / Patch run
   (see `sessions/flows/flow-ui-polish-session.md` → Decisions).
2. Should the dock's Debug tab filter to the selected node when one is selected (Node-RED
   doesn't, but it's cheap here via the existing feed)?
3. Selected-nodes export: include upstream `needs` references that point outside the
   selection, or strip them? (Recommend strip + warn count in the dialog.)

## Related

- `flows-canvas-scope.md` — the editor this polishes (import/export, config, undo).
- `flow-deploy-ux-scope.md` — defined the toolbar semantics this consolidates; the meanings
  of Deploy/Run/Enable are unchanged.
- `flow-runtime-control-scope.md` — `flows.node.update` (Save node) and mid-run lifecycle.
- `debug-node-scope.md` — the debug drawer becoming the dock's second tab.
- `docs/FILE-LAYOUT.md`, `docs/scope/testing/testing-scope.md`.
- Skill doc: **N/A** — no new agent-/API-drivable surface (UI recomposition only).
