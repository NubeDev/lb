# Frontend scope — shell chrome layout (header style + top-nav mode)

Status: scope (the ask). A frontend-only successor to the shipped `theme-customizer-scope.md`
Layout tab, alongside `theme-appearance-scope.md`. Promotes to `public/frontend/frontend.md`
once shipped.

Two new **appearance choices** for the app shell chrome, set from Settings → Theme → Layout and
persisted per-member through the same `ui_theme` prefs blob as every other Layout axis:

1. **Header style** — the page header can render as it does today (the icon-chip title band) OR as
   a standard **shadcn/ui `Breadcrumb`** trail (Workspace → Surface → …), a familiar
   admin-console pattern. Two values, one axis.
2. **Navigation mode** — the workspace nav can live in the left `Sidebar` as today OR as a **top
   menu bar with dropdowns** (shadcn/ui `Menubar`), a horizontal strip mounted *above* the header/
   breadcrumbs, with the sidebar's grouped sections becoming top-level menus and their entries the
   dropdown items. Two values, one axis.

Both are pure shell chrome, additive, and **use the shadcn/ui library primitives** (`breadcrumb`,
`menubar`, `dropdown-menu`) rather than hand-rolled markup — the same rule the existing Layout
controls follow (they map 1:1 onto the shipped shadcn `<Sidebar>`).

## Goals

- **Header-style axis.** A new closed axis `ThemeLayout.header: "band" | "breadcrumbs"`.
  - `band` (default) = today's `AppPageHeader` icon-chip band (unchanged — this is the current
    look and must stay pixel-identical when selected).
  - `breadcrumbs` = a shadcn `Breadcrumb` header rendering the trail
    `Workspace / <Surface> [/ <sub-page>]`, with the same top-right actions slot (workspace chip +
    Settings gear) the band header carries.
- **Nav-mode axis.** A new closed axis `ThemeLayout.nav: "sidebar" | "topmenu"`.
  - `sidebar` (default) = today's left `NavRail` (`<Sidebar variant/collapsible/side>`), unchanged.
  - `topmenu` = a horizontal `Menubar` above the header. Each `SURFACE_GROUPS` bucket (Workspace,
    Automation, Data, Build, System) becomes a `MenubarMenu` trigger; its surfaces become
    `MenubarItem`s. A resolved/curated nav (nav scope) renders the same way — top-level `group`s
    become menus, flat entries fold into a leading "Menu" trigger. Pinned favorites, the
    Extensions slots, and the Sign-out / Show-all-pages escape hatch all still appear, relocated
    into the menubar (e.g. a right-aligned overflow menu).
- **Live + persisted, like the other Layout axes.** Changing either re-lays-out the shell
  immediately (no reload) and persists through `prefs.set` on the `ui_theme` blob; it roams to
  every device and folds member → workspace-default → built-in exactly as the existing layout
  fields do.
- **Layout-tab UI.** Two new `OptionCard` groups on the existing `LayoutTab`, each with a small
  `SidebarMiniDiagram`-style thumbnail (a breadcrumb-vs-band diagram; a topbar-vs-siderail
  diagram), matching the Variant/Collapsible/Position controls already there.
- **Interaction between axes stays coherent.** When `nav === "topmenu"`, the sidebar-specific
  controls (Variant / Collapsible / Position) are still writable but visibly marked "sidebar only"
  (they no-op on the layout while the top menu is active) — no hidden state, no dead ends. The
  member can switch back to `sidebar` and their variant/side/collapsible choices are intact
  (they were never cleared).

## Non-goals

- **No new nav *content* model.** This does not change what the nav contains or how it's resolved
  (nav scope owns `nav.resolve`, cap-stripping, hide/pin). `topmenu` is a *second renderer* over
  the exact same resolved data (`ResolvedNavItem[]`, `SURFACE_GROUPS`, pins, ext slots) — not a new
  source of truth.
- **No breadcrumb *routing* engine.** The breadcrumb trail is derived from the already-known active
  `Surface` + workspace + any page sub-title the page already passes to `AppPageHeader`. We do not
  build a generic route-segment → label resolver or per-page crumb registry in v1; the trail is
  `Workspace / <surface label> [/ <page title>]`. A deeper multi-segment trail is a follow-up.
- **No backend, no new prefs axis key, no new cap, no new MCP verb.** Everything rides the existing
  `ui_theme` blob and the existing `prefs.set` / `prefs.resolve` verbs (see "How it fits the core").
- **No mobile/responsive redesign.** The top menu targets desktop widths; narrow-viewport
  collapse of the menubar (into a single "Menu" button) is a nice-to-have noted in Open questions,
  not a v1 requirement.

## Intent / approach

**One data axis per choice, two renderers, zero branches on identity.** The shipped Layout tab
already proves the pattern: `ThemeLayout` is a closed struct of enum axes, each validated in
`normalizeLayout`, each spread onto a shadcn primitive. We extend that struct with two more enum
axes (`header`, `nav`) and add:

- `theme-options.ts`: `HEADER_STYLES = ["band","breadcrumbs"]`, `NAV_MODES = ["sidebar","topmenu"]`,
  their types, their `DEFAULT_LAYOUT` values (`band` / `sidebar` — the current look), and their
  arms in `normalizeLayout` (unknown/absent → default, so **every stored theme keeps working**).
- A `HeaderBreadcrumbs` component (new file) beside `AppPageHeader`, chosen by
  `theme.layout.header` at the one place the header renders. The band path is literally today's
  component, untouched.
- A `TopMenuNav` component (new file) beside `NavRail`, fed the *same* props `NavRail` gets
  (`resolvedItems`, `allowed`, `extSlots`, `pinned`, handlers…). `RoutedShell` chooses rail-vs-topbar
  by `theme.layout.nav` and places the chosen one (left column vs. a top row spanning the content).

Why this shape (and the alternative rejected): the tempting shortcut is to make `topmenu` a *mode
flag read inside `NavRail`* that swaps its own markup. Rejected — it bloats one file past its one
responsibility (FILE-LAYOUT rule 8) and tangles two layouts' CSS. A sibling renderer over shared
data keeps each file to one job and lets the two evolve independently, exactly as `band` vs
`breadcrumbs` are two header files. The nav data (`ResolvedNavItem`, `SURFACE_GROUPS`, `itemRef`,
the pin grammar) is **already extracted** and consumed by `NavRail`; `TopMenuNav` imports the same
symbols. **No `if ext === …` branch is introduced** — ext slots stay opaque `ext:<id>` refs in the
menubar exactly as in the rail (CLAUDE rule 10).

**shadcn library components to add.** `breadcrumb`, `menubar`, and `dropdown-menu` are not yet in
`ui/src/components/ui/`. Add them via the shadcn CLI/registry (the same provenance as `sidebar.tsx`),
themed with the shell's global tokens — do **not** hand-author bespoke equivalents (ui-standards
scope; the ask explicitly says "make sure you use the library").

## How it fits the core

- **Placement:** UI-shell-only; runs identically on every node (no `if cloud`). N/A to node role.
- **Tenancy / isolation:** inherited — the theme prefs blob is already workspace+member scoped by
  the `prefs` verbs; this adds fields inside it, changing nothing about isolation. No new key.
- **Capabilities:** inherited — member persist is `mcp:prefs.set:call`; the admin workspace-default
  is `mcp:prefs.set_default:call` (a member lacking `set` degrades to local-only, exactly as today).
  No new cap. The deny path is the existing prefs deny path — a denied `prefs.set` leaves the choice
  local-cache-only, no crash.
- **MCP surface:** **none added.** This is a client-side render choice over the existing
  `prefs.set` (write member) / `prefs.set_default` (write workspace default) / `prefs.resolve`
  (read fold) verbs. Walking the four API shapes: **CRUD** — reuses `prefs.set`, no new write;
  **get/list** — reuses `prefs.resolve`/`prefs.get`, no new read; **live feed** — N/A (a preference,
  not a stream); **batch** — N/A. So the MCP surface section is deliberately empty of new tools.
- **Data (SurrealDB):** the two new fields live inside the opaque `ui_theme` prefs blob on the
  existing member/workspace prefs record. **No new table, no schema change** — the blob is
  host-opaque (see `prefs-closed-struct-not-kv` memory: this is a UI-side closed struct, the host
  stores it verbatim). State, not motion.
- **Bus (Zenoh):** N/A — no message class; a preference change is a local write + prefs record.
- **Sync / authority:** prefs are the authority; `theme-storage.ts` localStorage stays the
  first-paint cache so there's no flash on boot (the new fields normalize on read like the rest).
- **Secrets:** none.

## Example flow

1. An admin opens Settings → Theme → **Layout**. Below Position they now see **Header** (Band |
   Breadcrumbs) and **Navigation** (Sidebar | Top menu) `OptionCard` groups with thumbnails.
2. They pick **Breadcrumbs**. `setLayout({ header: "breadcrumbs" })` fires; the provider writes the
   new `ThemeLayout` into `ui_theme` via `persistTheme` and updates context. The page header
   instantly swaps from the icon-chip band to a `Breadcrumb` reading `acme / Settings`. The
   top-right workspace chip + Settings gear are preserved in the breadcrumb bar's actions slot.
3. They pick **Top menu**. `setLayout({ nav: "topmenu" })` fires. `RoutedShell` stops rendering the
   left `NavRail` and instead renders `TopMenuNav` as a horizontal `Menubar` above the header:
   *Workspace ▾ | Automation ▾ | Data ▾ | Build ▾ | System ▾* on the left, and a right-aligned
   overflow (Pinned, Extensions, Sign out). Opening *Automation ▾* drops down Rules / Flows /
   Reminders; clicking Flows navigates exactly as the rail entry did.
4. The Variant / Collapsible / Position cards now show a muted "sidebar only" hint (the top menu
   ignores them) but keep their values.
5. They set it as the workspace default via the tab's existing "Set as workspace default" action
   (`prefs.set_default`), so every member inherits Breadcrumbs + Top menu unless they override.
6. A member without `prefs.set` flips to Sidebar locally; it applies live but is cache-only (the
   prefs write is denied) — no error surfaced beyond the existing degrade path.

## Testing plan

Per `scope/testing/testing-scope.md`. This is a frontend, prefs-backed feature — exercise the
**real** gateway prefs path, no fakes (CLAUDE rule 9).

- **Unit (vitest):**
  - `theme-options.test.ts`: `normalizeLayout` fills `header`/`nav` from `DEFAULT_LAYOUT` when
    absent (an *old* stored theme with no header/nav field normalizes to `band`/`sidebar`) and
    rejects unknown values — the migration-safety guarantee.
  - `LayoutTab.test.tsx`: the two new `OptionCard` groups render, reflect the current axis, and
    call `setLayout` with the right patch; the "sidebar only" hint appears when `nav==="topmenu"`.
  - `TopMenuNav` render test: given a `resolvedItems` menu + `SURFACE_GROUPS` fallback, it emits the
    same set of navigable entries as `NavRail` (same `itemRef`s), and clicking an item calls
    `onSelect`/`onSelectDashboard`. Assert ext slots stay opaque `ext:<id>`.
  - `HeaderBreadcrumbs` render test: trail is `Workspace / <surface> [/ <page title>]`; actions slot
    still carries workspace chip + Settings link.
- **Gateway integration (`pnpm test:gateway`):** a `theme-prefs`-style test that sets a theme with
  `header:"breadcrumbs", nav:"topmenu"` via the **real** `prefs.set`, re-`resolve`s, and asserts the
  round-tripped blob preserves both new axes (proves no field is silently dropped — mirrors the
  `dashboard-variable-closed-struct` / `prefs-closed-struct` class of bug). Include the mandatory
  **capability-deny** case (a session without `prefs.set` gets a denied write, local-only) and the
  **workspace-isolation** case (member A's layout choice is not visible to member B in another
  workspace / does not leak across the wall).
- **Live verification (not just jsdom):** switch both axes in a running node and confirm the shell
  re-lays-out without reload and survives a refresh (prefs authority + localStorage cache), per the
  theme-appearance scope's "verify live" rule for chrome changes.
- **Hot-reload:** N/A (no extension instance state; pure UI).

## Risks & hard problems

- **The top menu must not lose any rail affordance.** The rail carries a lot: grouped sections,
  pins, ext slots, the active pill, the "Show all pages" / "Use my menu" escape hatch (no-lockout
  scope), Sign out, and per-icon colors. A menubar has less room — the risk is silently dropping
  one (e.g. the escape hatch), which would *trap* a user on a narrow curated nav with no way out.
  The top menu MUST surface the same escape hatch; enumerate every rail affordance and place each.
- **Breadcrumb trail truthfulness.** A crumb that links somewhere the user can't reach, or mislabels
  the surface, is worse than no crumb. Derive labels from the same `SURFACE_DEF` map the rail uses;
  don't invent a parallel label source.
- **Two-axis interaction.** `topmenu` + a `collapsible`/`variant`/`side` value must not produce a
  broken half-state (e.g. a collapsed phantom sidebar occupying space). The chosen renderer must be
  the *only* nav mounted — assert the rail is absent from the DOM in top-menu mode.
- **Migration.** Every existing stored `ui_theme` predates these fields. `normalizeLayout` must
  default them, tested explicitly — a missing default would flip existing users to an unintended
  layout on next load.
- **shadcn component drift.** Pulling `menubar`/`breadcrumb`/`dropdown-menu` from the registry may
  bring class names or a Radix version that needs the shell-token theming pass (like `sidebar.tsx`
  got). Budget for aligning them to the global tokens, not accepting stock styling.

## Open questions

- **Breadcrumb depth.** v1 is `Workspace / <surface> [/ <page title>]`. Do any surfaces (Dashboards
  → a specific board, Studio → a tab) want a real third segment now, or is that a follow-up? (Lean:
  follow-up; ship two-level first.)
- **Top-menu overflow placement.** Should Pinned + Extensions be their own menus, or fold into a
  single right-aligned "More ▾"? (Lean: Pinned and Extensions as their own menus when non-empty;
  Sign out + escape hatch in a right-aligned account/overflow menu.)
- **Collapsed narrow-viewport top menu.** Do we collapse the whole `Menubar` into one "Menu ▾"
  button below some width in v1, or defer responsive behavior? (Lean: defer; desktop-first.)
- **Does the top menu replace *or* co-exist with the sidebar's collapse toggle / brand?** The brand
  mark currently lives in the sidebar header and doubles as the collapse toggle. In top-menu mode
  where does the brand go — leading item of the menubar? (Lean: brand as the leading, non-menu
  element of the menubar.)

## Related

- `scope/frontend/theme-customizer-scope.md` — the shipped Layout tab this extends (the
  Variant/Collapsible/Position axes and the `ui_theme` persistence path).
- `scope/frontend/theme-appearance-scope.md` — sibling appearance axes on the same `ThemePreference`
  blob; the "verify live, not just jsdom" rule for chrome.
- `scope/frontend/nav-rail-scope.md` and `scope/nav/` — the resolved-nav data model both renderers
  consume (unchanged by this scope).
- `scope/frontend/shadcn-migration-scope.md` / `ui-standards-scope.md` — use library components,
  themed with shell tokens.
- `scope/prefs/prefs-scope.md` — the `prefs.set` / `set_default` / `resolve` verbs this rides; the
  closed-struct persistence rule (memory: `prefs-closed-struct-not-kv`).
- README `§3` (rules 8, 10), `§6.5` (UI shell). Code: `ui/src/lib/theme/theme-options.ts`,
  `ui/src/features/theme/LayoutTab.tsx`, `ui/src/features/shell/NavRail.tsx`,
  `ui/src/components/app/page-header.tsx`, `ui/src/features/routing/RoutedShell.tsx`.

## Skill doc

**N/A.** No new agent-/API-drivable surface — no MCP verb, no gateway route, no automatable task.
The feature is a client-side render choice over the existing `prefs` verbs, which already have
their own coverage. If a future slice exposes the layout as a driveable preset API, revisit.
