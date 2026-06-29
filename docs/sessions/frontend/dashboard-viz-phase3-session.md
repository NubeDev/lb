# Session — dashboard viz Phase 3 (backend-resolved transforms + datasource binding)

Status: **in-progress**

Scope: [`viz/README.md`](../../scope/frontend/dashboard/viz/README.md) Phase 3 —
[`transformations-scope.md`](../../scope/frontend/dashboard/viz/transformations-scope.md) (Part 1) +
[`datasource-binding-scope.md`](../../scope/frontend/dashboard/viz/datasource-binding-scope.md) (Part 2).
Builds on the shipped [Phase 1](dashboard-viz-phase1-session.md) + [Phase 2](dashboard-viz-phase2-session.md).
Public truth: [`public/frontend/dashboard.md`](../../public/frontend/dashboard.md) (Phase 3 subsection).

## The ask (exit gate, in my words)

Panel data stops being resolved on the client. A new pure Rust lib `lb-viz` (the `lb-prefs` twin) implements
Grafana's core transformer set over a canonical columnar `Frame`. A new host verb `viz.query(panel) -> {frames}`
(gated `mcp:viz.query:call`) dispatches each of the panel's `sources[]` targets **under the caller's authority**
(composing each target tool's own cap + the workspace wall), assembles frames, runs the `transformations[]`
pipeline via `lb-viz`, and returns canonical frames. The client's ONE data hook (`usePanelData`) swaps its body
to call `viz.query` — a one-file change. The editor's Query tab gains a datasource dropdown; the Transform tab
becomes a real pipeline editor writing `transformations[]` config (no client execution).

Invariant A: `usePanelData` stays the one hook. Invariant B: the transform pipeline is backend-only (`lb-viz`);
`views/reduce.ts` stays the per-viz frame→value reducer, NOT merged into the pipeline.

## Decisions (+ rejected alternatives)

1. **Target dispatch = re-enter `call_tool_at_depth` per target (depth+1).** `viz.query`'s resolver calls the
   host's existing generic MCP dispatcher (`tool_call.rs`) once per non-hidden target, passing the *caller's*
   principal + ws. This composes the target tool's OWN cap check, the workspace wall, and the exact
   store.query/series.*/federation.query routing (incl. the federation launcher) **for free** — no
   re-implemented dispatch, no render-path cap bypass. A denied/failed target → an honest empty frame, never a
   fabricated value or a host-privilege read.
   *Rejected:* matching `target.tool` inside the resolver and calling `store_query`/`federation_query` directly —
   more code, and it forks the canonical cap+routing path (a privilege-escalation risk the scope's Risks section
   flags). Re-entry is the one-impl choice.

2. **Datasource binding = "the tool the target names", dispatched under the caller.** A `DataSourceRef {type,uid}`
   is opaque on the host (panel-model already stores it opaquely). The client writes the resolved `tool` per the
   ref (`surreal`→`store.query`, `series`→`series.read`, `federation`→`federation.query`). The resolver just
   dispatches `target.tool`; the workspace wall + per-tool cap give isolation + federation leashing with no new
   render cap. A ws-B panel naming a ws-A `datasource:{ws}:{name}` fails the federation host check (ws is pinned
   from the token) — structural, not checked by viz.query itself.
   *Rejected:* a per-kind special binding on the cell (`cell.federationSource`) — forks the contract per kind
   (datasource-binding scope, "Rejected").

3. **Live/watch panels keep the shipped live SSE path; snapshot panels go through `viz.query`.** `viz.stream` is
   the scope's NAMED follow-up (not this phase). So `usePanelData` routes a `series.watch`/`bus.watch` target
   through the existing live `useSource` path, and everything else through `viz.query`. Documented honestly in
   the hook — no capability regressed, and `viz.stream` lands as the live successor.
   *Rejected:* routing everything through `viz.query` now — would regress live streaming until `viz.stream`.

4. **`federation.datasource.schema` (SQL-builder column dropdowns for an external source) is DEFERRED with an
   explicit scope note.** Phase 3 ships the datasource dropdown + federation *target resolution* end to end. The
   schema-dropdown verb is an addition to the federation plane (the binding scope itself flags it "for the
   federation-plane owner when Phase 3 lands") and needs a spawned external container; building it here would
   balloon the session. The builder falls back to the raw-SQL editor for a federation source until it lands.
   Named non-goal, not a silent gap (datasource-binding scope status updated).

5. **`viz.query` input = the inline panel spec** (`{ sources[], transformations[], varScope? }`) the client
   already holds, not a saved-panel ref. Matches the scope's "a panel spec OR a saved panel ref"; inline avoids a
   store read on every render and keeps the editor preview (an unsaved panel) working. Workspace comes from the
   token, never the spec.

## Build (vertical slice: lib → host verb → cap → client hook → editor)

- `rust/crates/viz/` (`lb-viz`): `frame.rs` (canonical Frame/Field + row↔frame adapter), `config.rs`
  (`Transformation`/`Matcher`, Grafana-verbatim ids+options), `transform.rs` (ordered pipeline dispatch),
  `transforms/<id>.rs` one per file: reduce, organize, filterFieldsByName, filterByValue, groupBy, joinByField,
  calculateField, sortBy, limit, merge, seriesToRows. Pure, no host/store deps. Unit-tested per transformer.
- `rust/crates/host/src/viz/`: `authorize.rs` (`mcp:viz.query:call`), `frame.rs` (rows→Frame at host edge),
  `query.rs` (resolver: dispatch targets via `call_tool_at_depth`, assemble, run `lb-viz`), `tool.rs`
  (`call_viz_tool` MCP bridge), `error.rs`, `mod.rs`. Wired into `tool_call.rs` (`viz.` host-native + branch).
- Client: `usePanelData.ts` body → `viz.query` (one file); `editor/tabs/QueryTab.tsx` datasource dropdown;
  `editor/tabs/TransformTab.tsx` real pipeline editor + `transformPipeline.ts` (the TS transform *registry*:
  ids + option editors — NOT an executor; invariant B).
- Follow-up (unblocked): `fieldconfig/format.ts` → real `format.*` MCP call behind the `viaPrefs` guardrail.

## Tests (green output pasted below on completion)

(pending)

## Green output

(pending)
