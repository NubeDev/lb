# frontend — library panels (session)

- Date: 2026-07-03
- Scope: ../../scope/frontend/dashboard/library-panels-scope.md
- Stage: S9+ collaboration UI (builds on the shipped v3 panel model + the `dashboard.*` asset/share model)
- Status: done

## Goal

Ship **library panels** end to end: panels become their own asset — a `panel:{id}` record holding the
**non-layout half of a v3 `Cell`** (the spec) — so a chart is (a) reusable across dashboards via a
`panel_ref` on `Cell` that the host hydrates at read, and (b) renders **standalone** on
`/t/$ws/panel/{id}` with no dashboard grid. The whole design keeps the **lens-vs-grant** boundary: a
shared panel shares its *definition*; its `sources[]` re-check under the **viewer's** caps at render.
Exit gate: the "sharing never widens data access" + cross-ws `panel_ref` no-hydrate headlines.

## What changed

**Backend — a new `panel` host module (`rust/crates/host/src/panel/`), cloned from `dashboard/`:**

- `model.rs` — `Panel` (`id/title/owner/visibility/spec/schema_version/updated_ts/deleted`), `PanelSpec`
  (exactly the non-layout half of a `Cell`, field-for-field), `PanelSummary` (roster: id/title/view/
  visibility/updated_ts — cheap, no usage count), `PanelUsageRow`.
- `store.rs` — raw read/write/scan for `panel` (envelope-unwrap mirrors `scan_dashboards`).
- The verbs, one per file, each its own cap: `get`, `list`, `save` (bounded via `bounds.rs`, which
  reuses the shared `dashboard::check_spec_bounds`), `delete` (delete-safety via `scan_usage`), `share`
  (S4 `share` edge), `usage`. `authorize.rs`/`visibility.rs`/`error.rs` are the three-gate check.
- **The two ref-lifecycle seams** the *dashboard* verbs call (host-side per the scope Decision):
  `hydrate.rs::hydrate_cells` (`dashboard.get` expands each ref cell → resolved v3 cell under the
  viewer's gates; dangling/unreadable → `panelMissing` placeholder, never a leaked spec) and
  `validate.rs::validate_and_strip_refs` (`dashboard.save` validates every ref resolves in-workspace —
  loud `BadInput` — and **strips the echoed spec**, so the ref is authoritative).
- `tool.rs` — the `call_panel_tool` MCP bridge.
- **Additive `Cell` fields** (`dashboard/model.rs`): `panel_ref`, `panel_vars` (bounded per-placement
  overrides), `panel_missing` (`skip_serializing_if`, never persisted). Inline cells deserialize
  unchanged; inline + ref cells coexist by design.
- Wired into `lib.rs`, `tool_call.rs` (`is_host_native` + dispatch), `system/catalog.rs` (6 host tools);
  `dashboard.get`/`dashboard.save` call the two seams.

**Gateway (`rust/role/gateway/`):** `routes/panel.rs` (6 routes: `/panels` CRUD, `/panels/{id}/share`,
`/panels/{id}/usage`), registered in `server.rs`; the six `mcp:panel.*:call` caps added to the dev-login
cap set.

**UI (`ui/`):** `lib/panel/` (types + `panel.*` api client + the `Panel↔Cell` bridge — `cellToSpec`/
`specToCell`/`refCell`/`unlinkCell` — + barrel); the `panel_*` cases in `lib/ipc/http.ts`; `CAP.panel*`
+ `CAP.dashboardGet` strings; additive `panelRef`/`panelVars`/`panelMissing` on the TS `Cell`. Editor
affordances: `editor/LibraryPanelBar.tsx` (Save-as-library / the "used on N dashboards" banner from
`panel.usage` / Save-to-library / Unlink) mounted in `PanelEditor`, and `editor/AddLibraryPanel.tsx`
(insert a ref cell from `panel.list`) beside "Add panel". `WidgetHost` renders the `panelMissing`
placeholder. The standalone page `features/panel/PanelPage.tsx` reuses `WidgetHost`/`usePanelData`/the
viz bridge (no parallel renderer) inside the shared `DashboardCacheProvider`, with its own range picker
+ `?var-` selections; routed at `/t/$ws/panel/$id` in `createAppRouter`, cap-gated on `panel.get`.

## Decisions made (recorded from the scope, all pre-resolved)

- **Hydration seam: host-side, on `dashboard.get`.** The UI editor uses `panelRef` as the link/unlink
  marker; the ref is authoritative (echoed spec stripped on save) — so headless callers/export get
  resolved dashboards for free and the deny/placeholder logic lives behind the wall once.
- **`dashboard.save` validates refs (loud), tolerates later dangling** (placeholder at hydration).
- **Standalone cap: distinct `panel.get`** (no piggybacking on `dashboard.list`).
- **No migrator** — additive `panel_ref`; a schema bump + re-seed is fair game (no prod data). Only
  inline+ref coexistence going forward is required.
- **Delete-safety decoupled from the `panel.usage` cap** — `panel.delete`'s pre-check uses a cap-free
  `scan_usage` (it's already gated on `panel.delete`), so delete never demands `panel.usage`.

## Tests (real store/caps/gateway, seeded records — rule 9)

**Backend `crates/host/tests/panel_test.rs` — 9/9 green:** crud round-trip; over-cap spec rejected;
capability-deny **per verb**; workspace-isolation (+ non-owner); gate-3 team-shared deny; the
**"sharing never widens data access" headline** (workspace-visible panel → viewer reads the definition,
`viz.query` returns EMPTY rows for the denied target, and `series.read` is denied directly); ref
hydration + inline/ref coexistence + propagation + **echoed-spec-ignored**; the **cross-ws `panel_ref`
rejected-at-save** headline + dangling → placeholder; delete-safety (in-use refused with the usage list
→ force tombstone → placeholder → re-save re-hydrates). No regression in `dashboard_test`/`nav_test`/
`dashboard_genui_test` (the additive `Cell` fields required updating three test constructors + the
shared bounds refactor).

**UI `features/panel/PanelPage.gateway.test.tsx` — 8/8 green (real spawned gateway):** CRUD round-trip;
capability-deny (no `panel.save`); workspace-isolation; save-as-library → reuse on a dashboard →
**edit-once-propagates** + echoed-spec-ignored; **cross-ws `panel_ref` rejected at save**; dangling →
`panelMissing`; share-definition-only; the **standalone page renders** full-bleed. `pnpm test` 430/430;
dashboard + nav gateway suites green (12) — hydration is a no-op for inline cells.

## Follow-ups / named deferrals (from the scope, unchanged)

No panel versioning/history (LWW v1); no "panel updated while you watch" bus push (re-hydrate on visit);
no cross-workspace panels (the wall); no channel/GenUI embed in v1 (the standalone route is the one new
render host); bulk import rides the viz import/export scope when it lands.

## Links

- Scope: [library-panels-scope.md](../../scope/frontend/dashboard/library-panels-scope.md) (open questions: None).
- Public: [public/frontend/dashboard.md](../../public/frontend/dashboard.md) → "Library panels".
- Skill: [docs/skills/panels/SKILL.md](../../skills/panels/SKILL.md).
