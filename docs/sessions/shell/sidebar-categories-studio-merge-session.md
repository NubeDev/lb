# Session: sidebar categories + Extensions/Studio merge

## Ask

Two UI-shell changes:
1. Reorganize the crazy flat sidebar into labelled categories (à la shadcn `sidebar-16`).
2. Merge the **Extensions** and **Studio** surfaces into one page, tabbed with **shadcn Tabs**,
   with the tabs driven by **routes** (deep-linkable).

## What shipped

### 1. Sidebar categories (`ui/src/features/shell/NavRail.tsx`)

The built-in fallback rail was one flat `SURFACES` list under a single "Core" label (17 items). Replaced
with `SURFACE_GROUPS` — the same surfaces bucketed into **Workspace / Automation / Data / Build /
System**, plus **Settings** relocated to the footer near Sign out. `SURFACES` is now derived by
flattening the groups so the `SURFACE_ICON` lookup that keeps the resolved rail and fallback in lockstep
is unchanged.

- A group whose members are all cap-stripped renders nothing (no orphan label).
- **Fallback only** — the server-authored `resolvedItems` (nav scope) path is untouched. It already
  supports one level of `group`; the long-term-correct model is the built-in rail being a sensible
  default the server nav can override, not forking category logic client-side.

### 2. Extensions + Studio → one tabbed "Studio" page

- New `ui/src/features/studio/StudioShell.tsx`: the merged page shell. shadcn `Tabs`, tabs are
  **route-driven** (active tab comes from the URL; `onSelectTab` navigates), so `/studio/extensions`
  and `/studio/build` are each deep-linkable and back/forward works — better than `AdminView`'s
  local-`useState` tabs.
- `ExtensionsView` and `StudioView` gained an `embedded` prop: when embedded they drop their own
  `AppPageHeader` (the shell supplies one) and surface their action (upload / "Start over") inline.
- Routing (`createAppRouter.tsx`): `/studio/extensions` + `/studio/build` routes, each behind its own
  `CoreGate` (its own cap). Bare `/studio` and legacy `/extensions` redirect to the first tab the
  session's caps allow (`StudioDefaultRedirect`).
- `surface.ts`: `extensions` → `/studio/extensions`, `studio` → `/studio/build`.
- Sidebar "Build" group collapses the two into a **single "Studio" entry** (keyed `extensions`),
  visible when **either** cap is allowed; its click routes through bare `/studio` so a build-only
  session lands on Build, not a denied Extensions tab (`RoutedShell.selectSurface`).

### Cap model (user decision)

**Keep both caps, one page.** `extensions` (ext.list) and `studio` (devkit.templates) stay distinct
CoreSurfaces. Each tab renders behind its own `CoreGate`; a session sees only the tabs it's permitted.
The gateway re-checks every verb regardless — hiding a tab is display convenience, not the boundary.

## Incidental fix

`ui/src/components/ui/tabs.tsx` called `React.useId()` **inside** a `useMemo` callback (rules-of-hooks
violation, warned in every Tabs consumer). Hoisted it to the top level. Pre-existing; surfaced by the
new StudioShell tests.

## Tests (green)

- `NavRail.test.tsx` (7): category grouping, empty-group hiding, merged Studio entry shows when either
  extensions **or** studio is allowed, Build group hidden when none allowed. Existing resolved-nav
  tests unchanged.
- `StudioShell.test.tsx` (4, new): allowed-tab rendering, per-cap tab hiding, `onSelectTab` wiring,
  active body.
- Full routing/shell/extensions/studio suite: 25 passed. Typecheck clean for all touched files (the two
  pre-existing flows/panel-builder test errors are not from this session).

## Rejected alternatives

- **Collapse to one `studio` surface** (drop the `extensions` CoreSurface): simpler but widens the
  security model (studio access would grant extension management) with a bigger blast radius across
  `allowed.ts` / `surface.ts` / gateway checks. Rejected for the two-caps-one-page model.
- **`<Link>`-wrapped tab triggers**: the codebase navigates imperatively via `useNavigate`; passing an
  `onSelectTab` callback from the route keeps the studio feature decoupled from router route types.
