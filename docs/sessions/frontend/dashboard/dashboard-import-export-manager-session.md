# Session — Dashboard import/export + a full-CRUD Dashboards manager (native bundle)

Date: 2026-07-09 · Branch: `cleanup-for-desktop` · Surface: `ui/` (dashboard feature)

## The ask

> Make a nice UX/UI manager for a dashboard to import/export one or more widgets/panels and/or the whole
> dashboard. Add a new page for all dashboards with full CRUD and an import/export. Admin user.

## What shipped

A **native, our-format** import/export for dashboards and standalone widgets, plus a **Dashboards
manager page** with full CRUD — all client-side, composing with the **already-shipped** `dashboard.*` /
`panel.*` verbs. **No new host verbs.** This is deliberately NOT the Grafana-JSON interchange
(`viz/import-export-scope.md`), which is a separate backend-mapped feature (`dashboard.import` /
`dashboard.export` host verbs, a bidirectional mapper). Ours is the "export from here, import here (or on
another node) under my own authority" artifact — the thing the user literally asked for — and it needs
zero backend work, so it ships today.

### The pieces (one responsibility per file)

- `ui/src/lib/dashboard/portable.ts` — the **portable bundle format** (`DashboardBundle`): a versioned,
  self-describing artifact (`kind: "lazybones.dashboard-bundle"`, `version: 1`, `.lbdash.json`) carrying
  one-or-more dashboards **and/or** one-or-more standalone widgets/panels. Pure: `makeBundle` /
  `serializeBundle` / `parseBundle` (validate, never guess) + id helpers (`bareId`, `slugFromTitle`,
  `uniqueId`). **Carries no workspace and no owner** — those are authority the import re-establishes from
  the token (rule 6). Ids are advisory; the importer re-slugs to a fresh non-colliding id so an import
  never silently overwrites.
- `ui/src/features/dashboard/io/downloadText.ts` — the DOM download step (object-URL + anchor), guarded
  for the non-DOM/test path.
- `ui/src/features/dashboard/io/useDashboardIo.ts` — the orchestration: **export** reads the selected
  records (`dashboard.get` / `panel.get`) → bundle → download; **import** replays a parsed bundle through
  `dashboard.save` / `panel.save`. Collision policy `rename` (default, collision-safe) vs `overwrite`
  (owner-only, re-checked host-side).
- `ui/src/features/dashboard/io/ImportDialog.tsx` — paste-or-upload → live preview (counts, per-entry
  titles, parse warnings) → collision choice → confirm → an honest outcome summary (created / renamed /
  errors). A Grafana JSON is turned away with a pointer, not half-imported.
- `ui/src/features/dashboard/manager/DashboardsManagerPage.tsx` — the **new full-CRUD page**: a filtered,
  sortable table of every reachable dashboard with create / rename / duplicate / delete (shared
  `ConfirmDestructive`) + multi-select **Export** + **Import**. Route `/t/$ws/dashboards/manage`.
- Entry points: a **Manage** button + an **Export this dashboard** icon in the `DashboardView` header, and
  a per-widget **Export widget** affordance in the grid cell hover cluster (`Grid.onExportCell` → the
  shipped `cellToSpec` Cell→PanelSpec bridge).

### Wiring

- `createAppRouter.tsx`: new `dashboardsManageRoute` (`/dashboards/manage`), reuses the `dashboards`
  surface cap (same feature, not a new `CoreSurface`); `DashboardsRoute` passes `onManage`.
- Barrels: `lib/dashboard/index.ts` re-exports `portable`; `features/dashboard/index.ts` exports the
  manager page.

## CLAUDE rules held

- **Rule 6 (workspace is the hard wall):** the bundle carries no ws/owner; import authority comes from the
  token. **Headline isolation test passes** — a bundle authored in ws-A imports into ws-B as a ws-B record,
  invisible to ws-A.
- **Rule 5/7 (capability-first, MCP contract):** import/export are ordinary `dashboard.save`/`.get` /
  `panel.save`/`.get` calls — each gated + re-checked host-side. Overwrite of a non-owner record is denied
  server-side; the UI gate is defense-in-depth.
- **Rule 9 (no mocks):** every test drives a **real spawned gateway** + real store; no `*.fake.ts`.
- **Rule 10 (core knows no extension):** the bundle treats ext-tile view ids (`ext:<id>/<widget>`) as
  opaque data — a widget's `view` string round-trips untouched; nothing branches on an extension id.

## Tests (all green)

- `lib/dashboard/portable.test.ts` (unit, 10) — round-trip, reject non-JSON / wrong-kind (Grafana) /
  newer-major / empty, skip-malformed-with-warning, id helpers.
- `features/dashboard/io/dashboardIo.gateway.test.tsx` (real gateway, 3) — seed→export→import round-trip
  through the real store; standalone widget replays through `panel.save`; **workspace isolation**.
- `features/dashboard/manager/DashboardsManagerPage.gateway.test.tsx` (real gateway, 2) — lists seeds +
  imports a pasted bundle into the table; creates from the toolbar.
- Regression: existing `DashboardView.gateway.test.tsx` (11) still green after the toolbar additions.

Commands: `pnpm test` (unit) and `pnpm test:gateway` (real node). `tsc --noEmit` clean; `eslint` clean
(the Grid raw-`<button>` warnings pre-exist that file's affordance pattern — the new export button follows
it 1:1).

## Rejected alternatives

- **Grafana-JSON verbs now** — rejected for this session: it's a large backend feature (mapper +
  schemaVersion migration + datasource remap) already scoped in `viz/import-export-scope.md`. The native
  bundle delivers the user's ask with zero backend risk and is orthogonal (a future Grafana import can land
  beside it). The bundle module's header comment points at that scope so the two don't get conflated.
- **A new `CoreSurface` for the manager** — rejected: it's the same dashboard feature; reusing the
  `dashboards` cap keeps nav/gating simple and honest.

## Follow-ups (not blocking)

- A nav entry / breadcrumb to the manager (today it's reached via the header **Manage** button + the
  `/dashboards/manage` deep link).
- Native Tauri save-dialog for export (today the webview anchor-download works; bytes are identical).
- A bulk "export all" one-click (today: select-all in the table then Export).
