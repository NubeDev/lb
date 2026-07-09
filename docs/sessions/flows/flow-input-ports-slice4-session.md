# Flows — port-labelled edges + per-input-port join policy, Slice 4 (the canvas) (session)

- Date: 2026-07-09
- Scope: ../../scope/flows/flow-input-ports-scope.md
- Stage: S8+ (flows shipped; this slice paints the input-port model on the canvas)
- Status: done (Slice 4 of 4 — the scope is **complete**)

## Goal

Paint the input-port + join-policy model (Slices 1–3) on the flow canvas (scope "Intent" step 8):
per-named-input-port handles on `FlowNodeView`, an `any` vs `all` glyph per port (funnel vs join),
the wire inspector shows `to_port`, and the palette shows a node's input ports + policies from the
real `flows.nodes` registry. `link-out`/`link-in` render as their built-in descriptors. UI-only — no
verb/descriptor/runtime change.

## What changed (Slice 4)

### Canvas ⇄ record (`flowGraph.ts`)
- New **`joinOf(desc, port?)`** + **`effectiveInputPorts(desc)`** — the canvas-side mirror of the
  host's `NodeDescriptor::join_of`: apply the per-kind default (`any` for a sink, `all` otherwise),
  then any explicit `inputPorts` override. Returns the resolved `{name, join}[]` a node renders.
- New **`CanvasInputPort`** type; `FlowNodeData` gained `inputPorts?: CanvasInputPort[]` (populated by
  `FlowCanvas` from the registry, the single source of truth for kind/ports).
- **`flowToEdges` now labels a named-port wire** with its `toPort` as a midpoint `label` — the "wire
  inspector" surface, so a multi-input node's wiring is legible at a glance without a separate panel.
  Primary-only wires stay label-free (the common case stays a clean canvas).

### The node (`FlowNodeView.tsx`)
- **Per-named-input-port handles.** A single-port node (the common case) keeps ONE anonymous handle
  (React Flow connects a null-`targetHandle` edge to it — back-compat with every existing flow). A
  multi-port node stacks one handle per declared port, vertically distributed; the **primary** port
  keeps the anonymous handle (a null `targetHandle` edge lands on it — the canvas convention), each
  **non-primary** port carries `id = portName` so its named wire matches.
- **The `any`/`all` glyph.** Every input handle wears a small lucide glyph just inside it: a `Filter`
  (the funnel shape) for `any` (fires per upstream — Node-RED OR), a `Merge` for `all` (barrier join).
  So at a glance the author reads `debug`/`link-in` = funnel, `rhai`/`avg` = join. A multi-port node
  also labels each handle with its port name.

### The palette (`Palette.tsx`)
- Each entry now shows its declared input ports with their effective policy: a tiny funnel/merge mark
  + the port name per port (e.g. `◇ payload` for `link-in`). An author picks a node **knowing whether
  it funnels or joins** before dragging it onto the canvas. A trigger/source (no inputs) shows none.

### Wiring (`FlowCanvas.tsx`)
- `paintedNodes` resolves each node's descriptor → `effectiveInputPorts(desc)` → `data.inputPorts`,
  alongside the existing `kind` resolution. One registry pass; no per-render allocation beyond it.

## Tests (real store/caps/gateway, rule 9) — all green

**UI unit (`pnpm exec vitest run src/features/flows`): 93 passed** (was 86; +7 in `flowGraph.test.ts`):
`joinOf`/`effectiveInputPorts` (defaults, override both directions, primary resolution, trigger→no
ports, link-in any) + the wire-inspector label (named-port labelled, primary-only clean). The 2
**pre-existing** `DebugValueView.test.tsx` reds remain (flagged NOT this scope — verified). `pnpm exec
tsc --noEmit` adds **no new flows errors** (the 18 pre-existing non-flows errors are unchanged).
`pnpm exec eslint` clean on touched files.

**UI gateway (real spawned node, `pnpm test:gateway`):**
- `FlowsCanvas.gateway.test.ts` **15/15** — incl. the new assertion that `link-out`/`link-in` ship as
  built-ins under a `Links` category in the **real** registry, and `link-in`'s `payload` port carries
  `join: "any"` (Slice 3/4 round-trip).
- `flowsDebug.gateway.test.ts` **2/2**, `FlowsRuntimeControl.gateway.test.ts` **6/6** — unchanged.

## Definition of done (Slice 4)

- [x] Per-named-input-port handles + the `any`/`all` glyph on the canvas; the wire inspector shows
      `to_port`; the palette shows input ports + policies; link nodes render as built-ins.
- [x] UI-only — no verb/descriptor/runtime change (rule 1/5/7 unaffected).
- [x] FILE-LAYOUT held (focused edits per file; `flowGraph` gained two named helpers, not a catch-all).
- [x] No core knowledge of any extension; no mock data / no fake backend.
- [x] Tests on the frontend (unit + gateway over the real registry), green output pasted.
- [x] Session doc + STATUS + public doc + scope close-out updated.

## The scope is complete

Slices 1–4 together deliver the whole `flow-input-ports` scope: the port-labelled edge model + the
per-port join policy (Slice 1), the `any` runtime + the propagated firing context (Slice 2), the
wireless `link` pair (Slice 3), and the per-port canvas (Slice 4). The headline Node-RED OR funnel
ships end to end — three wires into one `debug`/`link-in` print three times, multiplicity propagating
past the funnel, exactly-once per firing, per-firing deny + durable delivery holding. The scope's
status flips from "scope (the ask)" to **shipped**; the public doc (`doc-site/content/public/flows/
flows.md`) is the trimmed truth.
