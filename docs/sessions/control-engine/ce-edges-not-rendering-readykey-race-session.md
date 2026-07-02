# Session — CE wiresheet: edges (wire connections) never render

- **Date:** 2026-07-03
- **Branch:** `ce-node-wiring-v2` (stayed on it)

## The ask

"I can't see the react-flow node wire connections, they just don't show."

## What I found (real browser + real node, not inference)

Drove the live federated CE page with Playwright and read the DOM + `control-engine.tree`:

- the tree returns the edge correctly (`random.out(1000115) → dewpoint.rh(1000122)`);
- 7 nodes + 33 handles render, INCLUDING source handle `1000115` on node `100010` and
  target handle `1000122` on node `100011`;
- `buildRfEdges` therefore builds a correct RF edge — yet **0 `.react-flow__edge-path`**.

Adding a `console.log` inside the promotion selector made the edge appear → a **timing race**.

## Root cause

`CeEditor` parks new edges in `pendingEdges` and promotes them to RF's live `edges` only once
both endpoints' handles are registered (RF drops an edge whose handle isn't measured). The
gate was a `useStore(readyKey)` selector + a **fixed 1500ms grace timer from mount**. On a
cold federated-bundle load the selector didn't re-fire as `nodeLookup.internals.handleBounds`
populated (in-place mutation), and the grace fired before handles registered — so `readyKey`
stayed `""`, `pendingEdges` was nulled, and RF got zero edges. Warm loads won the race, hence
"sometimes wires, sometimes not".

## The fix

`packages/ce-wiresheet/src/CeEditor.tsx`:

- `readyEdgeIds()` reads handle bounds FRESH via `rf.getInternalNode(id).internals.handleBounds`
  (no stale `useStore` snapshot);
- promotion effect keys off `useNodesInitialized()` (RF's own "measured + handles wired"
  signal) + an rAF poll for bounds that land a frame later;
- the 1500ms grace-drop clock starts only AFTER `nodesInitialized` — before init, "unresolved"
  means "not measured yet", not "malformed", so a slow load gets its full time; a genuinely
  malformed edge (output→output) is still dropped after init.
- removed the now-unused `useStore as useRfStore` import from CeEditor.

## Verification (rule 9 — real browser, real node)

Playwright against the live federated page: `.react-flow__edge-path` 0 → 1 (the
`random.out→dewpoint.rh` wire), **stable across 3 consecutive cold loads**. New regression
`ui/e2e/ce-edges.spec.ts` asserts ≥1 edge path renders. A jsdom unit test can't reproduce a
handle-measurement-timing race, so the e2e is the honest regression.

Rebuild/publish chain (the ext bundle does NOT hot-reload): `pnpm build:lib` in
`packages/ce-wiresheet` → `pnpm build` in `rust/extensions/control-engine/ui` → copy `dist/*`
into `rust/extensions-ui/control-engine/`. `pnpm test` in ce-wiresheet: 153/153 green.

## Docs

- Debugging: [../../debugging/frontend/ce-edges-never-render-readykey-race.md](../../debugging/frontend/ce-edges-never-render-readykey-race.md) + a README history row.

## Note

The repo has an automation that periodically commits the working tree as "adding
external-agent" — it swept the CeEditor fix into commit `243856f` mid-session. The new e2e
spec + docs will be swept the same way. Not something this session triggered.
