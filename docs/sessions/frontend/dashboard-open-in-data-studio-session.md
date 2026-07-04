# Dashboard → Data Studio per-cell "Open" affordance (session)

Branch: `master`. Scope: `scope/frontend/dashboard/` + `scope/frontend/data-studio-scope.md`.
Public: `public/frontend/dashboard.md`, `public/frontend/data-studio.md`.

## The ask

Since data-studio v2, the dashboard no longer authors panels — it only **places** library panels
(ref cells) and **renders** them; the per-cell edit affordance was removed and the user was told to
"open it in Data Studio's Library pane and save it back". But there was no direct link from a
dashboard cell to the studio — the user had to know the studio existed, navigate there, and re-find
the panel in the Library pane. The owner asked for **a button on each dashboard panel that opens
Data Studio**, so editing a panel is one click away.

Decision (owner): the button appears on **every** cell (inline v1/v2/v3 + ref cells), and clicking
it **navigates to `/t/$ws/data-studio`** (no deep-link / panel-seeding in v1 — the user picks the
panel from the Library pane once there; deep-linking a specific panel into a builder tab is a named
follow-up on the data-studio `?openPanel=` seam).

## What shipped

### 1. `Grid.tsx` — an "Open in Data Studio" button in the cell hover affordance group

Each cell's top-right hover group (the existing `editable`-gated cluster that holds Duplicate /
Remove) gains a leading **`ExternalLink`** button:

- `aria-label="open cell ${c.i} in data studio"`, `title="Open in Data Studio"`;
- rendered only when `editable` (matches the existing duplicate/remove gating — editing is
  admin-only, `isAdmin(caps)`, the same `canEdit` that gates the whole authoring surface);
- calls a new optional `onOpenInDataStudio?: () => void` prop. Omitted ⇒ no button (the test seam —
  a harness that doesn't pass the callback renders no button, so the data-studio-v2 "no per-cell
  edit" REMOVAL REGRESSION still holds its shape).

The aria-label is deliberately **not** `edit cell …` — the data-studio-v2 regression test asserts
`queryByLabelText("edit cell w1")` is null (the in-place editor is gone), and that must stay true.
This button is a *navigation* affordance to the surface where editing happens, not an in-place
editor.

### 2. `DashboardView.tsx` — threads the callback

Adds an optional `onOpenInDataStudio?: () => void` prop and forwards it to `Grid`. Pure prop
threading — no logic.

### 3. `createAppRouter.tsx` (`DashboardsRoute`) — wires the navigation

`DashboardsRoute` holds two navigate hooks now:

- `searchNav = useNavigate({ from: "/t/$ws/dashboards" })` — unchanged, type-safe relative search
  updates (the existing `onSearchChange` path);
- `go = useNavigate()` — a bare navigate for the absolute cross-surface jump, mirroring how
  `SystemView` / `DatasourcesAdmin` navigate (`to: fullPathForSurface(ws, "data-studio")`).

*Why two hooks:* a single `useNavigate({ from })` is typed for its `from` route; calling it with an
absolute `to: string` to a *sibling* route fails typecheck (`Property 'search' is missing`), because
TanStack narrows NavigateOptions to the from-route's search contract. The bare hook has no such
narrowing. The split keeps each call site at its natural type-safety (search stays pinned; the
cross-surface jump is intentionally absolute).

No new host verb, cap, or table — pure UI navigation over the existing `/t/$ws/data-studio` route
(already cap-gated `data-studio` CoreSurface, re-checked by the route).

### 4. Test updates

`DashboardView.gateway.test.tsx`:

- Both render harnesses (`renderDashboard`, `renderDashboardWithSearch`) now pass a stub
  `onOpenInDataStudio={() => {}}` so the button renders in tests.
- The data-studio-v2 REMOVAL REGRESSION test was updated: the in-place editor is still absent
  (`queryByLabelText("edit cell w1")` is null) AND the new navigation affordance is present
  (`getByLabelText("open cell w1 in data studio")`). The comment was rewritten to reflect that the
  dashboard carries a *navigation* affordance to the studio, not an *authoring* surface in-place.

No new test file — this is a one-line navigation callback; the regression assertion that the button
exists (and the existing 11-test gateway suite staying green) is the coverage. A click-through test
(navigate → a builder tab seeded with the cell) is deferred with the deep-link follow-up.

## How it fits the core

- **Capabilities (rule 5):** the button is `editable`-gated (`isAdmin(caps)`), so it's shown only to
  editors — the same gate as every other authoring affordance on the dashboard. The destination
  (`/t/$ws/data-studio`) is independently cap-gated by its route (`ctx.allowed.includes(
  "data-studio")`); an editor without that surface hits the standard `DefaultRedirect`.
- **Core knows no extension (rule 10):** `data-studio` is a `CoreSurface`, not an extension id; the
  navigation goes through the generic `fullPathForSurface(ws, surface)` seam — no branch on an ext.
- **One responsibility per file (rule 8):** `Grid` owns the affordance, `DashboardView` threads the
  prop, `DashboardsRoute` owns the navigation. No file grew past its role.
- **No mocks (rule 9):** the regression test runs against the real gateway + real seeded series +
  real `panel.save`/`dashboard.save` write paths; the button assertion is on a real rendered cell.

## Verification

- `pnpm tsc --noEmit` — clean for the touched files (3 pre-existing unrelated errors remain).
- `pnpm lint` — the new button adds one `no-restricted-syntax` *warning* (raw `<button>`), matching
  the existing move/duplicate/remove buttons in the same cluster (the file's convention for the cell
  chrome; the shadcn `<Button>` primitive is heavier chrome than a 6×6 hover icon). No new errors.
- `pnpm test:gateway` (DashboardView) — **11/11 green**, incl. the updated regression test.
- `pnpm test` (unit) — **547/547 green**.

## Open questions / follow-ups

- **Deep-link a specific panel into a builder tab** — `?openPanel=<id>` (ref cell) / `?openCell=
  <base64>` (inline) read on Data Studio mount → opens a builder tab seeded via `specToCell` /
  `draftFromSelection`. The data-studio scope already names this as the natural next step; the
  callback seam here (`onOpenInDataStudio: () => void`) widens to `onOpenInDataStudio: (cell?) =>
  void` with no Grid change. Named follow-up, not an open question.
- **Show the button to viewers** — today it's `editable`-gated. The data-studio surface itself is
  member-level, so a read-only viewer *can* reach it (and explore data there). Widening the button
  to all viewers is a one-line change (`editable` → always) if the owner wants it; kept editor-only
  for now to match the existing hover-affordance cluster.
