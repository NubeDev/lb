# Session — dashboard surface (Phase 1: grid + built-in widgets over real series)

Topic: `frontend` · Scope: [dashboard-scope.md](../../scope/frontend/dashboard-scope.md) (Phase 1) ·
Date: 2026-06-27 · State: **shipped**

## The ask

Build the first-party dashboard surface from `dashboard-scope.md` **Phase 1**: a logged-in user opens a
workspace, sees dashboards of its real series, drags/resizes widget tiles on a grid, and watches charts
update live — everything against a **real node** (real store, real series, real cap checks, real Zenoh
motion), seeded with **real records** through the real ingest path (no fleet, no mocks; CLAUDE §9). The
user asked to "get coding on the frontend part for the dashboard and widgets as needed" — so this slice
builds the whole Phase-1 vertical (store → cap → MCP → gateway → SSE → UI), because the frontend can't be
real without the backend verbs (the no-fakes rule). **Phase 2 (federated widgets) was deliberately not
started** — it's the security-load-bearing federation surface and warrants its own reviewed slice; Phase 1
proves the binding contract first, exactly as the scope sequences it.

## What shipped (vertical slice, build steps 1–6)

**Backend (Rust):**

1. **`seed_iot_demo`** (`crates/host/src/dashboard/seed.rs`) — emits real `Sample`s for `cooler.temp`
   (oscillating °C) and `fryer.state` (on/off) via the **raw ingest path** (`lb_ingest::write` →
   `commit_batch`) and tags the series entities over the **real tag graph** (`kind`/`store`/`equipment`),
   so `series.read`/`series.find` return them. A dev/test seed (no principal — it seeds its own
   workspace, like `lb_inbox::record` in the harness), not an MCP verb.
2. **`dashboard` host service** (`crates/host/src/dashboard/`) — one verb per file: `get`/`list`/`save`
   (UPSERT create+update, owner-only update)/`delete` (idempotent tombstone)/`share` (sets visibility +
   writes the S4 `share` edge). `authorize.rs` is the gate-1+2 chokepoint (`mcp:dashboard.<verb>:call`
   via `authorize_tool`); `visibility.rs` is the **gate-3** membership/visibility resolver reusing the
   shipped S4 `share`/`member` edges (`lb_assets::list_related`) — a non-member of a `team` dashboard is
   denied, exactly the S4 doc-sharing deny extended to dashboards. `store.rs` is the typed read/write +
   scan-roster seam. `tool.rs` is the MCP bridge (`call_dashboard_tool`). 5 new caps.
3. **Series motion** (`crates/host/src/ingest/motion.rs`) — `publish_sample` (writes a `Sample` onto the
   workspace-scoped bus subject `ws/{id}/series/{series}`) + `subscribe_series`/`SeriesSub` (authorize
   `series.read` → subscribe). **This was the one piece the scope assumed existed but didn't** — nothing
   published series motion before. State stays the committed `series` table; motion is best-effort (rule 3).
4. **Gateway routes** — `routes/dashboard.rs` (`GET|POST /dashboards`, `GET|DELETE /dashboards/{id}`,
   `POST /dashboards/{id}/share`) each re-checking the three gates server-side, ws+owner from the **token**
   (§7); `routes/series_stream.rs` (`GET /series/{series}/stream` SSE, `?token=` auth, the series analog of
   the channel stream). The existing `POST /ingest` route now **publishes motion** for each written sample
   so a live widget sees it advance. A test-only `/_seed/iot_demo` route in the harness.

**Frontend (React/TS):**

5. **`lib/dashboard/`** — `dashboard.api.ts` (verbs mirror the routes 1:1 through `invoke`),
   `dashboard.types.ts` (Cell/Dashboard/Binding/Visibility), `series.stream.ts` (`openSeriesStream` over
   EventSource, `?token=`). `http.ts` gained the 5 `dashboard_*` verbs.
6. **`features/dashboard/`** — `DashboardView` (roster + selected grid + share/delete header),
   `DashboardRoster` (list + create), `AddWidget` (palette: type + series **or** tags binding), `Grid`
   (`react-grid-layout`, layout ↔ `cells[]`, drag/resize-stop persists via `dashboard.save`), `WidgetHost`
   (dispatch by `widget_type`; `ext:<id>` is the Phase-2 seam), `useDashboard` (roster/select/save/delete/
   share), `useSeries` (resolve binding → backfill `series.read` → fold live SSE samples), and the three
   built-in widgets (`ChartWidget` SVG line, `StatWidget`, `GaugeWidget` SVG arc — hand-drawn, no charting
   dep). Cap-gated **Dashboards** nav slot (`App.tsx`/`NavRail`/`admin-caps`). Added dep:
   `react-grid-layout` (+ a local ambient `.d.ts` since upstream ships no types and the `@types` stub is
   deprecated).

## Decisions / notes

- **Phase 2 not started — by design.** Federated widgets (`dashboard-widgets-scope.md`) are the trust
  boundary; Phase 1 ships the binding contract (`{widget_type, binding}` + 4 read verbs) first-party so
  Phase 2 is a renderer swap. `WidgetHost`'s `default` branch is the placeholder seam.
- **Series motion was missing** and is the only backend addition beyond the scope's named verbs. Published
  from the gateway write route (it holds `node.bus`); the producer on the live frame is stamped to the
  token's principal to match the committed row. Multiplexed stream stays a named follow-up.
- **Roster via `lb_store::scan`** (one capped page, `MAX_SCAN_LIMIT`), unwrapping the `{data:…}` envelope
  `write` adds. A paged roster for >200 dashboards is the named follow-up the scope already lists.
- **`save` preserves owner + visibility** (owner-only update); visibility changes only via `share` — so a
  layout save never silently re-privatizes a shared dashboard.

## Tests (all green)

- **Host** (`crates/host/tests/dashboard_test.rs`, 5): CRUD round-trip · deny-per-verb · **gate-3
  team-shared member-reads/non-member-denied** + roster membership-filter · two-ws isolation +
  non-owner-cannot-overwrite · **seed integrity** (`series.read` returns 24 cooler samples, `series.find`
  resolves the tagged series).
- **Gateway** (`role/gateway/tests/dashboard_routes_test.rs`, 6): CRUD round-trip over the real router ·
  save-without-cap → 403 server-side · two-session ws isolation · workspace-visible read by another member
  (gate-3) · series stream **401 without token** · **live `sample` over a real socket** (open SSE → publish
  → received).
- **UI Vitest, real gateway** (`features/dashboard/DashboardView.gateway.test.tsx`, 3): create → add chart
  bound to a real seeded series → renders + **persists** (re-render reloads from the store) · **tag-bound**
  stat widget resolves via `series.find` · workspace isolation (a fresh workspace shows no dashboards).
- Full suites: `cargo test --workspace`, `cargo fmt --check`, `pnpm test` (20), `pnpm test:gateway` (56),
  `pnpm build` (tsc + vite) — all green.

## Follow-ups (named, not silent)

- **Phase 2** — widgets as federated extensions (the frozen contracts in `dashboard-widgets-scope.md`).
- Multiplexed series stream (one SSE, many series); paged roster (>200); dashboard table into the §6.8 sync
  set; multi-admin live layout refresh; per-dashboard/team-scoped sharing primitives (Phase 1.5).
- Tauri desktop command layer for `dashboard_*` (the browser/gateway path is the one shipped).
