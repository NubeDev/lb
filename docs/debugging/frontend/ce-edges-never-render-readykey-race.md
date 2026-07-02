# CE wiresheet: edge (wire) connections never render — promotion gate raced the grace timer

- **Date:** 2026-07-03
- **Area:** frontend / ce-wiresheet (React Flow edge promotion)
- **Status:** resolved
- **Branch:** `ce-node-wiring-v2`

## Symptom

The Control Engine canvas rendered its nodes (function blocks) with all their handles, but
**no edges** — the wire connecting `random.out → dewpoint.rh` never drew. No console error;
the badge and values were fine (a separate fix). Just: zero wires.

## Investigation (real browser, real node)

Drove the live federated page with Playwright and read the DOM + the `control-engine.tree`
response:

- `control-engine.tree` returned the edge correctly: `{sourceUid:100010, sourcePropertyUid:1000115, targetUid:100011, targetPropertyUid:1000122, sourceProperty:"out", targetProperty:"rh"}`.
- 7 nodes rendered; **33 handles** rendered — including the two the edge needs:
  handle `1000115` = **source** on node `100010`, handle `1000122` = **target** on node `100011`.
- `buildRfEdges` therefore produced a correct RF edge (`source:"100010", sourceHandle:"1000115", target:"100011", targetHandle:"1000122"`).
- Yet **`.react-flow__edge-path` count was 0.**

The tell: adding a `console.log` inside the promotion selector made the edge appear. A
behavior that changes when you add a log is a **timing race**.

## Root cause

`CeEditor` parks new edges in `pendingEdges` and promotes them to React Flow's live `edges`
array only once both endpoints' handles are registered in RF's internal store — necessary
because RF drops an edge whose handle isn't measured yet. Promotion was gated by:

1. a `useStore` selector (`readyKey`) that walked `s.nodeLookup.get(id).internals.handleBounds`
   and returned the ready edge ids as a joined string, and
2. a **fixed 1500ms grace timer** (`setTimeout(() => setPendingEdges(null), 1500)` from the
   moment `pendingEdges` was set) that dropped anything still unresolved as "malformed".

On a **cold federated-bundle load** (the CE page is a lazily `import()`-ed remote — first
paint is slow), two things went wrong together:

- the `useStore` selector did not re-fire to a non-empty value as handle bounds populated
  (RF mutates `nodeLookup` in place; the nested `internals.handleBounds` change didn't
  reliably re-run the selector), and
- the 1500ms grace fired **before** the handles finished registering, nulling `pendingEdges`.

Net: `readyKey` stayed `""`, the grace dropped the edge, and RF was handed zero edges — every
wire silently vanished. On a warm/fast load the handles won the race, which is why it was
intermittent and looked like "sometimes wires, sometimes not".

## Fix

`packages/ce-wiresheet/src/CeEditor.tsx` — replace the fragile selector + fixed timer with a
readiness read that cannot go stale and a grace clock that cannot start too early:

- **Read handle bounds FRESH**, not through a subscription: `readyEdgeIds()` calls
  `rf.getInternalNode(id).internals.handleBounds` (the same `{source:[{id}], target:[{id}]}`
  RF itself uses) each time it runs — no stale `useStore` snapshot.
- **Drive promotion off `useNodesInitialized()`** — RF flips it to `true` once every node is
  measured and its handles are registered, so the effect re-runs at exactly the right moment
  regardless of how slow the cold load is — plus an **rAF poll** to catch handle bounds that
  populate a frame or two after `nodesInitialized`.
- **Start the grace clock only after `nodesInitialized`.** Before init, "unresolved" just
  means "handles not registered yet", not "malformed"; gating the 1500ms drop on
  `nodesInitialized` gives a slow load its full measurement time and only then drops a
  genuinely-unresolvable (malformed, e.g. output→output) edge.

## Verification (real browser, rule 9)

Playwright against the live federated page + real node: before the fix `.react-flow__edge-path`
= 0 on every cold load; after the fix = 1 (the `random.out → dewpoint.rh` wire), stable across
3 consecutive cold loads. Regression test: `ui/e2e/ce-edges.spec.ts` logs in, opens the CE
page, and asserts at least one edge path renders. A jsdom unit test cannot stand in — the race
depends on real handle **measurement** timing, which only a layout engine produces.

Note: the CE page is a built federated bundle (`remoteEntry.js`) — it does NOT hot-reload from
source. Reproducing/fixing needs `pnpm build:lib` in `packages/ce-wiresheet` → `pnpm build` in
`rust/extensions/control-engine/ui` → copy `dist/*` into the served
`rust/extensions-ui/control-engine/`. A stale bundle will show old behavior even with fixed
source (this cost real time mid-investigation — the byte-identical rebuild "fixed then
un-fixed" it, which is what exposed the race as the true cause).

## Lesson

A promotion gate that depends on a subscription re-firing AND a fixed timer from mount is a
race waiting for a slow load. When you must wait for a framework to finish measuring
(handles, layout), key off the framework's own "initialized" signal and read state FRESH at
that moment — never assume a `useStore` selector re-runs on an in-place nested mutation, and
never start a "give up" timer before the thing you're waiting for could even have happened.
