# Session — dashboard viz Phase 3 (backend-resolved transforms + datasource binding)

Status: **shipped (2026-06-29)**

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

## Tests

Real infra, seeded real rows, no mocks/no `*.fake.ts` (testing-scope §0/§3.1).

- **lb-viz units (49)** — each transformer over canonical frames incl. the mandatory empty/non-numeric →
  honest `Null` (never a fabricated 0); the Matcher (byName/byType/byRegexp) cases.
- **`crates/host/tests/viz_query_test.rs` (7, real Node + store + caps):**
  - `store_target_with_pipeline_returns_expected_frames` — store target + `filterByValue`→`sortBy` → exact frame.
  - `no_transform_panel_parity` — viz.query rows EQUAL a direct `store.query` (the swap is invisible).
  - `multi_target_join_assembles` — two targets + `joinByField` → one joined frame.
  - `viz_query_denied_without_cap` — MANDATORY: no `mcp:viz.query:call` → opaque `Denied`.
  - `denied_target_is_honest_empty_not_a_bypass` — viz.query granted but `store.query` not → empty, no bypass.
  - `workspace_isolation` — ws-B sees none of ws-A's rows; ws-A sees its own (token-derived wall).
  - `federation_bound_target_resolves_through_federation_query` — a federation-bound target routes through
    the gated `federation.query`; an unregistered source → honest empty (no cross-tenant leak).
- **Gateway `viz.phase3.gateway.test.tsx`** — usePanelData renders a seeded panel via viz.query identically
  to Phase 2; Transform-tab authoring; viz.query-deny stays denied. (Required `mcp:viz.query:call` in the
  dev-session `member_caps` — the new member-level render path; see the debug entry.)
- **UI unit (147)** unchanged green; **dashboard_test (10)** + **gateway lib (2)** unchanged green.

### Bug found + fixed this session

The `usePanelData`→`viz.query` swap moved rendering onto a new gated verb absent from the dev session's
`member_caps()` → dashboard gateway panels rendered empty. Fixed by adding `mcp:viz.query:call` to
`member_caps` (+ granting the seed caps in the new test). Regression-covered by the host deny/isolation
tests + the gateway render test.
[`debugging/frontend/gateway-seed-series-500-denied-preexisting.md`](../../debugging/frontend/gateway-seed-series-500-denied-preexisting.md).

## Green output

```
### lb-viz ###            test result: ok. 49 passed; 0 failed
### lb-host viz_query ###  test result: ok. 7 passed; 0 failed
### lb-host dashboard ###  test result: ok. 10 passed; 0 failed
### gateway lib ###        test result: ok. 2 passed; 0 failed
### cargo build --workspace ### Finished (clean)
### cargo fmt --check (lb-viz, lb-host, lb-role-gateway) ### clean
### UI pnpm test ###       Test Files 22 passed (22) · Tests 147 passed (147)
### UI tsc --noEmit ###    clean (exit 0)
### UI pnpm test:gateway ### 161 passed | 1 failed (the pre-existing SystemView sheet flake — passes 9/9
                             isolated; every viz/dashboard/editor case green)
```

The one full-run gateway failure (`SystemView > opens the subsystem detail sheet`) is the pre-existing
flake named in the Phase-3 brief — it passes 9/9 isolated and is not this slice.

## Deferred (named, not silent)

- **`viz.stream`** — live frames over SSE (so live panels don't re-transform client-side either). Phase 3
  ships the snapshot `viz.query`; a `series.watch`/`bus.watch` panel keeps the shipped live path meanwhile.
- **`federation.datasource.schema`** — SQL-builder column dropdowns for an external source (a federation-
  plane add, needs a spawned container). The Query tab uses the raw-SQL editor for a federation source now.
- **`format.ts` → real `format.*` MCP call** — its own session ([`format-prefs-swap-followup.md`](format-prefs-swap-followup.md))
  because `formatValue` is synchronous at ~13 render callsites (sync→async cascade).
