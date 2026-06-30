# Flows — the React Flow canvas (slice E, Wave 3)

- Area: flows
- Status: shipped (green)
- Scope: [`scope/flows/flows-canvas-scope.md`](../../scope/flows/flows-canvas-scope.md).
- Spine: [`scope/flows/flows-scope.md`](../../scope/flows/flows-scope.md) (Decisions **1, 3, 12**
  editor-side).
- Session: this file. Prev: [extension + triggers](extension-triggers-session.md) (backend spine).
  Sibling: [dashboard-binding](dashboard-binding-session.md) (slice F).

## What this slice is

The **editor surface** — the React Flow canvas a user authors, edits, runs, and watches. It
**generalises the shipped rules-workbench chain canvas** from the chain `Step` to the typed `Node`
model, and it adds **no new authority**: it is a pure client of the shipped `flows.*` / `flows.nodes`
gateway verbs (no new host verbs, no new caps, no new tables). The headline is the two pieces of
legibility the scope names:

1. **No hardcoded UI** — one generic `SchemaForm` renders *every* node's settings from its
   descriptor's inline JSON-Schema 2020-12 and validates with `ajv` before `flows.save`. A new
   extension node gets a config form for free.
2. **Versioning made visible** — the v-pinned banner + the executed-node-lock + `flows.patch_run`
   (config-only, unexecuted nodes) dissolve the edit-while-running footgun (Decision 1).

## What shipped

### Gateway routes (`rust/role/gateway/src/routes/flows.rs`) — one verb per route, mirroring `chains.rs`
- `/flows` GET (list) · POST (save) · `/flows/nodes` GET (the merged registry / palette source) ·
  `/flows/{id}` GET/DELETE · `/flows/{id}/run` POST · `/flows/{id}/enable` POST ·
  `/flows/{id}/inject` POST · `/flows/{id}/runs` GET (reattach) · `/flows/runs/{run_id}` GET
  (snapshot) · `/flows/runs/{run_id}/{op}` POST (`suspend`/`resume`/`cancel`) ·
  `/flows/runs/{run_id}/patch` POST (`flows.patch_run`). Each re-checks `mcp:flows.<verb>:call`
  server-side via `lb_host::call_tool`; workspace + principal from the **token** (§7). An invalid DAG
  or schema-invalid node config → `400` with the message verbatim (the canvas inline error).
- Registered in `server.rs` + `routes/mod.rs`. NO new authority — these are the shipped `flows.*`
  verbs over the gateway. The dev member caps (`session/credentials.rs`) gained the `mcp:flows.*`
  set (member-level, like `chains.*`); the UI `CAP.flowsList` gates the nav entry.

### UI client (`ui/src/lib/flows/`) — one call per export, 1:1 with the routes
- `flows.types.ts` — mirrors the host `Flow`/`Node`/`NodeDescriptor`/`FlowRunSnapshot` shapes
  (camelCase, the wire truth). `flows.api.ts` — one export per verb. `http.ts` gained the
  `flows_*` command→route mapping (browser path).

### Canvas (`ui/src/features/flows/`) — one component/hook per file, ≤400 lines
- `flowGraph.ts` — the canvas⇄record serialization (Flow→React-Flow nodes/edges and back, 1:1; the
  run snapshot → colour map; the executed-node set the editor locks).
- `SchemaForm.tsx` — the JSON-Schema 2020-12 renderer (object/string/number/integer/boolean/enum/
  nested-object/array-of-scalars) + `validateConfig` (ajv 2020-12 + ajv-formats). A descriptor that
  exceeds the covered subset **fails loud** ("unsupported schema") — never a silently-dropped field.
- `Palette.tsx` — renders `flows.nodes` grouped by `category`; drag-drop or click adds a node
  instance.
- `FlowNodeView.tsx` — the custom React Flow node (typed, coloured by run status, the executed-node
  lock, the shown-but-gated mark).
- `NodeConfigPanel.tsx` — the SchemaForm host + Save / Patch-run gating on ajv validity + the
  executed-node-lock read-only render.
- `useFlowRun.ts` — the bounded `flows.runs.get` poll (stop on terminal, hard ceiling) + `reattach`
  via `flows.runs.list {status:"active"}` on open.
- `FlowCanvas.tsx` — the canvas: edit/save (new version)/run/suspend/resume/cancel/patch_run/import
  /export/undo + the v-pinned banner.
- `FlowRail.tsx` + `useFlows.ts` (roster + palette, refetch on focus = the hot-reload claim) +
  `FlowsView.tsx` (thin page wrapper). Registered as the `flows` core surface (NavRail + routing +
  `allowed.ts` + `surface.ts`).

### Tests (real, no mocks)
- **`SchemaForm.test.tsx`** (unit, 8): ajv accepts valid; rejects missing-required, out-of-range enum
  (the `qos:9` example), wrong-type; no-schema accepts anything; renders string/enum/boolean; fires
  onChange; renders an inline field error.
- **`FlowsCanvas.gateway.test.tsx`** (real gateway, 13): palette = built-ins + a seeded real extension
  node grouped by category; save round-trip + faithful canvas serialization; invalid DAG rejected
  inline (`/cycle/i`); schema-invalid node config rejected inline; run → runs.get colours the node;
  import/export round-trip through save validation; **undo restores a deleted node + its edges
  atomically**; workspace-isolation (ws-B can't get/run ws-A's flow; roster partitioned);
  **capability-deny** (a viewer without `flows.save` sees the palette but save is refused, nothing
  persisted); inject-retain starts no run; patch_run wired to the host gate; delete idempotent.
- **`flows_routes_test.rs`** (real gateway, 7): nodes returns the 5 built-ins; CRUD round-trip; cyclic
  DAG → 400 inline; run → runs.get snapshot; save denied without the cap; workspace-isolation (absent
  flow collapses to opaque 403, no existence leak); inject-retain → `fired_run:false`.

Green output:

```
ui: SchemaForm.test.tsx (8 passed) · FlowsCanvas.gateway.test.tsx (13 passed)
    pnpm test → 176 passed · pnpm lint → 0 errors
rust: flows_routes_test → 7 passed · cargo test --workspace → exit 0 (all results ok)
      cargo fmt --all --check → clean
```

## Decisions made this slice

- **Undo is client-side (transient edit history), not the store journal.** The shipped undo
  auto-capture's reversible floor is `inbox.record`-shaped single-record upserts; `flows.save`
  classifies as `NonGeneric` and is NOT journaled reversible (it writes the whole flow as one record
  via `write`, not `write_journaled`). So the canvas keeps a transient undo stack of the prior graph
  (component state — the scope's "transient unsaved buffer" allowance, NOT client-durable graph
  state) and "undo" re-saves the previous version. The atomicity the scope requires holds because a
  flow is one record — undo of "add node + edges" reverts both in a single `flows.save`. **Host-side
  `flows.save` journaling via `write_journaled` is a named deferral** (traces to the undo scope,
  still "building") that would make this the store journal for free; the client undo is swapped
  1:1 then.
- **`flows.watch` SSE deferred (Decision-traced, named in the spine).** The canvas uses the bounded
  `flows.runs.get` poll (the scope's named fallback). The SSE route — mirroring the channel/agent run
  streams — removes the poll when it lands.
- **`flows.get` on an absent/tombstoned flow collapses to opaque `Denied`** (the host's
  existence-hiding discipline), surfaced as `403` at the gateway — NOT `404`. The canvas renders this
  as "not permitted" / not-found identically (the picker only lists reachable flows).
- **`flows.patch_run` renders the current descriptor's schema.** The fully-correct Decision-12 form
  renders the run's *pinned* schema; the host validates against the pinned schema and a mismatch
  surfaces as an honest `400` inline. A future pinned-schema fetch (a `flows.runs.get` field) makes
  the form pin-exact; the deny is correct either way.

## Open questions / follow-ups
- `flows.watch` SSE (removes the poll).
- Host-side `flows.save` journaling → swap the client undo to the store journal for free.
- A pinned-schema field on `flows.runs.get` so the patch form is pin-exact.
- Per-resource flow narrowing rides the platform-wide `authz-grants` follow-up (Decision 7).
