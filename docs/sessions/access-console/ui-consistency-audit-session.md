# Access console — UI/UX consistency audit + remediation (session)

- Date: 2026-07-08
- Area: `ui/src/features/admin/` (the Access console tabs)
- Stage: post-S8 (UI polish; no backend change)
- Status: done — Roles pilot + all 6 sibling tabs converged; `AdminPanel` retired

## Goal

The Admin section had grown inconsistent across its 7 tabs (Overview, People, Teams,
Roles, Workspaces, API Keys, Nav): the same "list + New X" idea was expressed three
different ways, some tabs used the shared shadcn `Table` and others hand-rolled raw
`<table>`, one tab hardcoded a `red-500` literal (violating the token rule), no table
header stayed put while rows scrolled, and there was no search anywhere. Audit what
worked, then converge every tab on one pattern using the already-committed primitives
(shadcn `ui/*`, app shell `app/*`) — no new libraries.

## What was produced

- **Two shared building blocks**
  - `ui/src/components/ui/table.tsx` — added an opt-in `sticky` prop to `TableHeader`
    (`sticky top-0 z-10 bg-panel shadow-[0_1px_0_hsl(var(--border))]`, the `DataView`
    treatment). Backward-compatible (default off).
  - `ui/src/features/admin/AdminToolbar.tsx` — **new** shared toolbar: left search
    `Input` (gated by `onSearch`) + right action slot (the "New X" `Button`). The one
    top-bar pattern for every list tab.

- **Shared scroll fix (root cause of the "nothing scrolls" bug)**
  - `ui/src/components/ui/tabs.tsx` — `TabsContent` rendered a plain **block** div, so a
    `min-h-0 flex-1` panel inside it (and the intervening `<Reveal>` motion wrapper) had
    no flex parent to resolve height against → content overflowed the viewport with no
    scroll region. Fix: `TabsContent` is now `flex min-h-0 flex-col`, and the `Reveal`
    wrapper carries `flex min-h-0 flex-1 flex-col`. This completes the flex-height chain
    (`Tabs → TabsContent → Reveal → panel → overflow-y-auto`). Safe: Rules/Studio bypass
    `TabsContent` entirely, so only Admin (which uses it) is affected.

- **RolesAdmin (the pilot — worst offender)**: retired `AdminPanel`, raw `<table>` →
  shared `Table` (sticky header), raw `<button>`/`<input>`/checkbox → shadcn
  `Button`/`Input`/`Checkbox`, fixed `w-1/2` → responsive `flex-col md:flex-row`, search
  on both the role list and the ~209-cap checklist. Made the editor a **response** (a
  placeholder until you select a role or click "New role"), so "New role" now visibly
  opens the create form (with Cancel) instead of appearing inert. `Plus` icon on New.

- **Phase B — the same recipe applied to the rest**
  - `WorkspacesAdmin.tsx` — retired `AdminPanel`; raw `<table>` → shared `Table`
    (sticky); **removed the `bg-red-500/15 text-red-400` literal** — Purge is now
    `Button variant="destructive"`, Archive `variant="outline"`; search over ws/name.
  - `ApiKeysAdmin.tsx` — retired `AdminPanel`; raw `<table>` → shared `Table` (sticky);
    "New key" moved into the toolbar with the `Plus` icon; search over label/prefix.
  - `PeopleAdmin.tsx`, `TeamsAdmin.tsx` — adopted `AdminToolbar` (search + `Plus`),
    sticky header, filtered rosters, a `min-h-0 flex-1 overflow-y-auto` roster scroll
    region so the sticky header pins.
  - `nav/NavAdmin.tsx` — adopted the `AdminToolbar` header row (`Plus` New nav) + roster
    search.
  - **Deleted `AdminPanel.tsx`** — no importers remained.

## Decisions

- **Fix the scroll bug in the `TabsContent` primitive, not per-panel.** It was a latent
  bug for every `TabsContent` consumer; short content just hid it. One shared fix beats
  seven `flex flex-col` sprinkles, and Rules/Studio (which don't use `TabsContent`) are
  untouched.
- **Editor-as-response for Roles** (placeholder until action) over an always-on form —
  otherwise "New role" set the same state already showing and looked dead. Matches the
  People/Teams master-detail placeholder ("No X selected").
- **`Plus` as the universal New-icon.** The tabs previously used mismatched icons
  (person-add / people / key / none). `Plus` reads as "create" and is now uniform.

## Tests (real gateway, CLAUDE §9 — no fakes)

- `cd ui && pnpm exec vitest run --config vitest.gateway.config.ts src/features/admin/`
  → **10 files / 35 tests pass**. Updated one selector: the RolesAdmin "no caps to
  bundle (no-widening)" test now clicks "New role" first (the editor opens on action).
- `tsc --noEmit` clean.
- Manual: each tab shows one header (no double header), a sticky table header while rows
  scroll, a working filter, "New X" in the same place, and no `red-*` literal
  (`grep -rn "red-" ui/src/features/admin` → none).

## Polish round (post-review, same session)

Screenshots surfaced three "these look different" issues; fixed:

- **"＋ Cancel" bug** — the New button kept the `Plus` icon when its label flipped to
  "Cancel". Now the icon shows only in the "New X" state and the open state is a clean
  `outline` "Cancel". Fixed in `ApiKeysAdmin`, `PeopleAdmin`, `TeamsAdmin`.
- **Nav editor looked like a different surface** — it opened straight to a bare "← Back"
  with no toolbar, and Nav's whole layout was inset by a root `p-4` so its toolbar didn't
  align with the other tabs. Restructured `nav/NavAdmin.tsx`: root is now flush
  `flex h-full min-h-0 flex-col`, both roster and editor lead with a full-bleed
  `AdminToolbar`, and only the scroll body below carries `p-4`.
- **Loud filled chips** — the API-key kind/role selectors (and the Webhooks auth-mode
  selectors) used the `solid` (filled teal) variant when selected, the loudest element on
  the screen. Switched selected → `default` (accent *tint* + accent text + subtle border),
  matching the quieter console. Only admin touched; the app-wide `solid` uses (Studio,
  dashboards, query-workbench) are their own features and were left.

Decisions (with the user):
- **Dropdowns stay native.** The `Select` primitive is a native `<select>` (token-styled
  closed control; OS menu when open) used in 32 files app-wide — the deliberate
  accessible/mobile choice. Making Nav a `Combobox` would make Nav the lone exception.
  Migrating everything to `Combobox` is noted as a separate app-wide task, out of scope.
- **Disabled ≠ glass.** The washed-out "Create key" / "Apply visibility" buttons are the
  normal `disabled:opacity-50` state (empty required field / unsaved nav), not a style bug.

## Roles capability tree (follow-up, same area)

The Roles editor's 209-item `mcp:…:call` checklist was flat and un-navigable, and Save sat
below all 209 rows. Redesigned:

- **New pure helper** `ui/src/features/admin/roles/groupCaps.ts` (+ `groupCaps.test.ts`, 8
  cases) — buckets caps by their first id-segment (the extension). `mcp:` prefix + `:call`
  suffix stripped for the display label; wildcards → `*` group; non-`mcp:` caps → `other`.
  Deterministic ordering (named groups alphabetical, `*` then `other` last).
- **`RolesAdmin.tsx`**: the checklist is now **collapsible groups** (Radix `Collapsible`),
  each with a `checked/total` badge and an All/None button. Rows show the short label
  (`def.list`) with the full cap in `title` + the (unchanged) `include ${cap}` aria-label.
  Groups start **collapsed** but auto-expand when they hold a checked cap or match the filter
  (best-practice: compact overview, never hides selections). **Save moved into a sticky header**
  so it's reachable without scrolling.
- **Load-bearing detail:** `CollapsibleContent forceMount` + `data-[state=closed]:hidden`
  keeps every checkbox mounted while collapsed, so the gateway test's
  `getByLabelText("include mcp:user.manage:call")` (which never expands the group) stays green.

Tests: `groupCaps` 8/8; `RolesAdmin.gateway.test.tsx` 3/3; `tsc` clean. No test edits needed.

## Second pass (2026-07-08) — floating panel, alignment, Workspaces create

Screenshot-driven polish, all in `ui/src/features/admin/`. The reusable recipes extracted
from this pass live in **`docs/scope/frontend/admin-page-patterns-scope.md`** (P1–P4 + a
pre-flight checklist) — read that to apply the same look to the next console.

- **Nav roster → shared `Table`.** `nav/NavAdmin.tsx` rendered its roster as a `<ul>` of
  cards; converted to the shared `Table` (Title / Visibility / Id / Actions) so it matches
  API Keys and the other list tabs. Empty/filtered states use `AppEmptyState`.

- **Nav "New nav" keeps the toolbar mounted.** The Nav editor used to replace the whole
  roster (including its search bar). Extracted the editor JSX to a `navEditor` const and made
  the `AdminToolbar` always-mounted; its action flips **New nav ⇄ Cancel** and the editor
  renders below it — mirroring the API Keys create flow (pattern **P4**).

- **Floating content panel (pattern P1).** The Access console content sat flat on `bg-bg`
  while the tab strip was a floating rounded-bordered pill, so the content read as a square
  with an "invisible" border. Wrapped all `TabsContent` in one panel matching the tab-strip
  material (`rounded-lg border border-border bg-panel-2/70 overflow-hidden`), floated on a
  shared `px-2`/`m-2` gutter so the strip and content align as two floating panels. Change is
  in `AdminView.tsx` only — the shared `Tabs` primitive is untouched (scoped, per CLAUDE §10).
  Verified with a token-accurate static render (not the live app; the change is CSS-only and
  the component tree is covered by the passing gateway tests).

- **Table left-padding consistency.** API Keys and Nav wrapped their scroll container in
  `px-4`, indenting their tables ~16px more than Workspaces/People/Teams/Roles (which are
  edge-to-edge). Removed the container padding from both; moved the inset onto the non-table
  children (`mx-4 mt-3` on the create form / secret banner). All six tables now share one left
  edge (pattern **P2** invariant).

- **Workspaces "New workspace" (net-new).** The Workspaces tab was lifecycle-only
  (list/rename/archive/purge); creation lived only in the switcher. Added the create flow
  (pattern **P4**): a `create(ws, name)` method on `useWorkspacesAdmin.ts` wired to the
  existing real `workspace.create` verb (`ui/src/lib/workspace/workspace.api.ts`), a new
  `CAP.workspaceCreate` (`mcp:workspace.create:call`) in `admin-caps.ts`, and a toolbar
  button + id/name form in `WorkspacesAdmin.tsx` gated by `hasCap`. `AdminView.tsx` now passes
  `caps` to the tab. New gateway test drives the form against the real node and asserts the row
  lands in the live directory. The verb gates against the caller's own workspace server-side
  (unchanged — same as the switcher).

- **List/detail bar alignment (pattern P3).** In Roles, the roster's `AdminToolbar` and the
  editor's header sat at different heights, so their rows didn't line up. Root cause: the
  editor header was a **sticky element with `-mt-4`/`-mx-4` negative margins inside a `py-4`
  scroll pane**, which offset its effective top from the flush left toolbar. Restructured the
  right panel to be structurally identical to the left — a plain first-child bar
  (`min-h-12 py-2 border-b bg-panel`) then a separate `overflow-y-auto` body. Also pinned the
  shared `AdminToolbar` to `min-h-12` so bar height is content-independent everywhere. Save
  stays visible while the cap tree scrolls (the bar is outside the scroll region).

Tests (second pass): AdminView 4/4, ApiKeysAdmin 2/2, NavAdmin 5/5, WorkspacesAdmin 3/3,
PeopleAdmin 3/3, TeamsAdmin 2/2, RolesAdmin 3/3; `tsc --noEmit` clean.

## Follow-ups

- Overview (card grid) was already clean; left as-is.
- The `solid` Button variant on the API-key kind/role selectors is the only `solid`
  usage; kept (valid variant, not a violation).
- **Promote `admin-page-patterns-scope.md` to `public/frontend/`** once a second console
  (settings, an extension admin UI) adopts P1–P4 — that proves the recipes generalize.
- Consider adding a **rename** row-action to Workspaces (`useWorkspacesAdmin` already exposes
  `rename`, which also un-archives); the table currently shows only Archive/Purge.
