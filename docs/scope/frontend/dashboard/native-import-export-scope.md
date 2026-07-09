# Dashboard scope — native bundle import/export + the Dashboards manager

Status: **SHIPPED (2026-07-09)**. Session:
[`sessions/frontend/dashboard/dashboard-import-export-manager-session.md`](../../../sessions/frontend/dashboard/dashboard-import-export-manager-session.md).
Durable facts promoted to [`public/dashboard/dashboard.md`](../../../../doc-site/content/public/dashboard/dashboard.md).

One paragraph: deliver "**import/export one or more widgets/panels and/or the whole dashboard**, and a
**page for all dashboards with full CRUD**" as a **pure client-side, our-format** feature that composes
with the **already-shipped** `dashboard.*` / `panel.*` verbs — **no new host surface**. A dashboard (or a
standalone widget) exports to a versioned `.lbdash.json` **bundle** that carries the portable shape only
(title, cells, variables, per-widget spec) and **never a workspace or owner**; import replays it through
`dashboard.save` / `panel.save` **under the caller's token authority** (rule 6), re-slugging ids so an
import never silently overwrites. A new `/t/$ws/dashboards/manage` page is the full-CRUD library table +
the import/export toolbar.

## Distinction from Grafana JSON (do not conflate)

This is **NOT** [`viz/import-export-scope.md`](viz/import-export-scope.md). That is a separate **backend**
interchange — two host verbs (`dashboard.import` / `dashboard.export`) + a bidirectional Grafana↔our-record
mapper + a `schemaVersion` migration + datasource remap. This native bundle is the **our-format** artifact
for moving dashboards/widgets between our own dashboards, workspaces, and nodes with zero backend work. The
two are orthogonal and can coexist: a future Grafana import lands beside the native bundle, not on top of
it. The `portable.ts` header comment states this so the boundary stays visible.

## The bundle (the interchange contract)

`lib/dashboard/portable.ts` (pure — no I/O, no React, no `invoke`):

- `kind: "lazybones.dashboard-bundle"`, `version: 1` (`BUNDLE_VERSION`), ext `.lbdash.json`.
- `dashboards: PortableDashboard[]` (`{ id, title, cells, variables?, schemaVersion? }`) **and**
  `panels: PortablePanel[]` (`{ id, title, spec, schemaVersion? }`) — either or both; a valid bundle has
  ≥1 entry across the two.
- **No `workspace`, no `owner`, no `visibility`, no timestamps** — authority + volatile fields the import
  re-establishes, never carries.
- `parseBundle` **validates, never guesses**: rejects bad JSON, missing/wrong `kind` (a Grafana export is
  turned away with a pointer), a MAJOR version it can't read, and an empty bundle; skips a malformed entry
  with a warning and imports the rest.
- Ids are **advisory**: `uniqueId` re-slugs on collision (`-2`, `-3`, …) so `rename` mode never overwrites;
  `overwrite` keeps the id but the host still enforces owner-only.

## Surfaces

- **Manager** (`features/dashboard/manager/DashboardsManagerPage.tsx`, route `/dashboards/manage`): a
  filtered/sorted table of every reachable dashboard; create / rename / duplicate / delete (via
  `ConfirmDestructive`), multi-select **Export**, **Import**. Reuses the `dashboards` surface cap.
- **Import dialog** (`features/dashboard/io/ImportDialog.tsx`): paste-or-upload → preview (counts, titles,
  warnings) → collision choice → confirm → honest outcome summary.
- **Header + grid entry points** (`DashboardView`, `Grid`): **Manage** button, **Export this dashboard**,
  and per-cell **Export widget** (→ the shipped `cellToSpec` Cell→PanelSpec bridge).

## How it fits the core

- **Tenancy (rule 6):** bundle carries no ws/owner; import authority = the token. Isolation test: a ws-A
  export imports into ws-B as a ws-B record, invisible to ws-A. **Mandatory test — passes.**
- **Capabilities (rule 5/7):** export = `dashboard.get`/`panel.get`; import = `dashboard.save`/`panel.save`
  — each gated + re-checked host-side. Non-owner overwrite denied server-side.
- **Extensions (rule 10):** a widget's `view` (incl. `ext:<id>/<widget>`) is opaque data that round-trips
  untouched; nothing branches on an extension id.
- **Data / bus:** import creates ordinary `dashboard:{id}` / `panel:{id}` records via the shipped UPSERT;
  no new table, no bus traffic. State vs motion holds.

## Testing (real gateway, no fakes — CLAUDE §9)

- Unit: `lib/dashboard/portable.test.ts` (round-trip, reject cases, id helpers).
- Gateway: `features/dashboard/io/dashboardIo.gateway.test.tsx` (store round-trip, widget replay,
  **workspace isolation**) + `features/dashboard/manager/DashboardsManagerPage.gateway.test.tsx` (list +
  import + create).

## Follow-ups (not blocking)

- A nav/breadcrumb entry to the manager (today: header **Manage** button + deep link).
- Native Tauri save-dialog for export (today the webview anchor-download works).
- Grafana-JSON interchange stays its own scope ([`viz/import-export-scope.md`](viz/import-export-scope.md)).
