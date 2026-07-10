# Shell chrome layout — header style + top-nav mode (session)

Scope: [`docs/scope/frontend/shell-chrome-layout-scope.md`](../../scope/frontend/shell-chrome-layout-scope.md)
Status: **shipped** (2026-07-10). Two new appearance axes on the existing Layout tab + the `ui_theme`
prefs blob, both additive and migration-safe.

## What shipped

Two new **closed axes** on `ThemeLayout`, set from Settings → Theme → Layout, persisted/roamed through
the same `ui_theme` blob as every other Layout axis (no new verb/cap/table — reuses `prefs.set` /
`prefs.set_default` / `prefs.resolve`):

1. **Header style** — `ThemeLayout.header: "band" | "breadcrumbs"`.
   - `band` (default) = today's `AppPageHeader` icon-chip band, **pixel-identical, untouched**.
   - `breadcrumbs` = a **clean shadcn `Breadcrumb`** header (the shadcn look exactly — no icon chip,
     no gradient/liquid-glass wash, no description sub-line; just the trail `Workspace / <Surface>`
     with the trailing actions slot preserved: workspace chip + Settings gear).
2. **Navigation mode** — `ThemeLayout.nav: "sidebar" | "topmenu"`.
   - `sidebar` (default) = today's left `NavRail`, unchanged.
   - `topmenu` = a horizontal shadcn `Menubar` mounted **above** the content; the left `NavRail` is
     omitted entirely (the chosen renderer is the *only* nav mounted — no phantom collapsed sidebar).
     Each `SURFACE_GROUPS` bucket becomes a `MenubarMenu`; its surfaces become `MenubarItem`s. A
     resolved/curated nav renders the same way — flat entries fold into a leading **Menu** trigger,
     top-level `group`s become their own menus. Pinned favorites + Extensions get their own menus when
     non-empty; the no-lockout escape hatch (**Show all pages** / **Use my menu**) + **Sign out** live
     in a right-aligned account menu.

## Files (FILE-LAYOUT: one verb per file)

**Theme layer (the closed-struct axes):**
- `ui/src/lib/theme/theme-options.ts` — `HEADER_STYLES`/`NAV_MODES` enums, `HeaderStyle`/`NavMode`
  types, `ThemeLayout.header`/`.nav` fields, `DEFAULT_LAYOUT` (`band`/`sidebar`), and the
  `normalizeLayout` arms (unknown/absent → default — the migration-safety guarantee).
- `ui/src/lib/theme/index.ts` — re-exports the new symbols.

**shadcn library primitives (themed to the Lazybones shell tokens):**
- `ui/src/components/ui/breadcrumb.tsx` — the `breadcrumb` primitive (shadcn new-york; reaches
  `@radix-ui/react-slot` — no new radix dep; structural markup).
- `ui/src/components/ui/dropdown-menu.tsx` — the `dropdown-menu` primitive over
  `@radix-ui/react-dropdown-menu` (NEW dep).
- `ui/src/components/ui/menubar.tsx` — the `menubar` primitive over `@radix-ui/react-menubar`
  (NEW dep).

**The two renderers (sibling files, zero branches on identity):**
- `ui/src/components/app/header-breadcrumbs.tsx` — `HeaderBreadcrumbs`, the alternative page-header
  style. Pure shadcn breadcrumb; the actions slot is preserved (parity with the band header).
- `ui/src/features/shell/TopMenuNav.tsx` — `TopMenuNav`, the alternative nav renderer. Fed the
  EXACT same props `NavRail` gets; ext ids stay opaque `ext:<id>` refs (CLAUDE §10).
- `ui/src/features/shell/nav-item-ref.ts` — extracted the shared `itemRef` (the hide/pin grammar)
  so both renderers agree on refs; `NavRail` re-exports it (no copy-paste drift).

**Wiring (the one place each choice lands):**
- `ui/src/components/app/page.tsx` — `AppPage` reads `theme.layout.header` via **`useThemeOptional`**
  (falls back to `band` when no provider — keeps standalone `/panel` renders working) and chooses
  `AppPageHeader` vs `HeaderBreadcrumbs`.
- `ui/src/features/routing/RoutedShell.tsx` — reads `theme.layout.nav`; `sidebar` mounts `NavRail`,
  `topmenu` mounts `TopMenuNav` above the content (the rail is absent from the DOM in top-menu mode).
- `ui/src/features/shell/index.ts` — barrel re-exports `TopMenuNav`.

**The Layout tab + diagrams:**
- `ui/src/features/theme/LayoutTab.tsx` — two new `OptionCard` groups (**Header style**,
  **Navigation**) above the existing sidebar axes; the "sidebar only" hint appears on the
  variant/collapsible/position hints when `nav === "topmenu"` (values kept, not cleared).
- `ui/src/features/theme/layout/SidebarMiniDiagram.tsx` — `HeaderDiagram` (band-vs-breadcrumbs) +
  `NavDiagram` (sidebar-vs-topmenu) thumbnails.

## Decisions (open questions resolved as recommended)

- **OQ1 (breadcrumb depth):** deferred — v1 ships the two-level trail `Workspace / <Surface>`. A
  deeper multi-segment trail (Dashboards → a specific board, Studio → a tab) is a follow-up; the
  surface label + an optional page sub-title is enough for v1.
- **OQ2 (top-menu overflow placement):** resolved as recommended — **Pinned** and **Extensions** as
  their own `MenubarMenu`s when non-empty; **Sign out** + the escape hatch in a right-aligned
  account/overflow menu.
- **OQ3 (collapsed narrow-viewport top menu):** deferred — desktop-first; responsive collapse into a
  single "Menu ▾" is a follow-up.
- **OQ4 (brand placement in top-menu mode):** resolved as recommended — the brand is the **leading,
  non-menu element** of the menubar (static `BrandHeader` with `canToggle=false`: there's no sidebar
  to collapse in top-menu mode).

## Key implementation notes

- **`useThemeOptional` in AppPage** — the strict `useTheme` throws outside a `ThemeProvider`. Some
  gateway tests (and the standalone `/panel/{id}` render) mount `AppPage`-bearing views without a
  provider. The optional hook falls back to `band` (today's look) so those paths stay green and the
  header choice is a graceful enhancement, never a crash.
- **Two renderers, not a mode flag.** The tempting shortcut was a `topmenu` flag read inside
  `NavRail` that swaps its own markup. Rejected (per the scope) — it would bloat one file past its
  one responsibility (FILE-LAYOUT rule 8) and tangle two layouts' CSS. A sibling renderer over shared
  data keeps each file to one job. The nav data (`ResolvedNavItem`, `SURFACE_GROUPS`, `itemRef`, the
  pin grammar) was already extracted; `TopMenuNav` imports the same symbols. No `if ext === …` branch.
- **`itemRef` extracted.** Previously a local function in `NavRail`; now `nav-item-ref.ts` so both
  renderers share the one mapping (a pin toggled in one is the same ref in the other). `NavRail`
  re-exports it — its existing consumers are untouched.
- **Pin toggle in the top menu.** The rail is the primary pin surface (its hover-toggle affordance
  has no clean menubar equivalent). The top menu reads pin state (a `Pin` glyph on pinned items) and
  adds a **Pin current page / Unpin current page** action in the account menu when `onTogglePin` is
  present — so the affordance is reachable, not lost.
- **shadcn primitives themed to shell tokens.** `breadcrumb`/`menubar`/`dropdown-menu` use the
  Lazybones token classes (`bg-panel`/`text-fg`/`border-border`/`text-muted`/`accent` on hover-focus)
  rather than stock shadcn `bg-popover`/`text-popover-foreground` — aligned to the global tokens like
  `sidebar.tsx`. Provenance is the shadcn new-york registry source; classes adapted, not hand-rolled
  bespoke equivalents.

## Tests (real store/bus/gateway/caps — rule 9)

- **Unit (vitest), 25 files / 127 tests green** across `lib/theme` + `features/theme` + `features/shell`
  + `components/app`:
  - `theme-options.test.ts` (+4): `normalizeLayout` fills `header`/`nav` from `DEFAULT_LAYOUT` when
    absent (an old stored theme stays put), rejects unknown values, preserves explicit values, and
    keeps the other layout axes when only header/nav are malformed — the migration-safety guarantee.
  - `LayoutTab.test.tsx` (+2): the Header + Navigation `OptionCard` groups drive `setLayout`, reflect
    the axis, and the "sidebar only" hint appears when `nav==="topmenu"` (and switching back to
    `sidebar` keeps the sidebar-axis values intact — no hidden state lost).
  - `TopMenuNav.test.tsx` (8): the fallback `SURFACE_GROUPS` as menus, cap-stripped-group omission,
    onSelect navigation, the resolved menu (flat → "Menu", groups → own menus), Pinned/Extensions
    menus, opaque ext ids, the escape hatch + Sign out in the account menu, hidden-set subtraction.
  - `header-breadcrumbs.test.tsx` (4): the `Workspace / <title>` trail as a shadcn breadcrumb
    (`aria-current="page"` on the page crumb), no-workspace path, the actions slot (workspace chip +
    Settings link) parity, and the trailing actions slot.
  - `NavRail.test.tsx` (14, unchanged): `itemRef` extraction did not regress the rail.
- **Gateway (`pnpm test:gateway`), `theme-prefs.gateway.test.ts` 6/6:** the WIDENED blob now carries
  `header:"breadcrumbs", nav:"topmenu"` and the round-trip test asserts both survive a fresh-boot
  re-resolve (the prefs-closed-struct class of bug). The existing **capability-deny** (a member
  without `prefs.set` is denied persist) and **workspace-isolation** (ws-A's theme never resolves in
  ws-B) cases cover the new axes by riding the same prefs path.
- **`tsc --noEmit` clean.**

## Live verification (manual)

Switched both axes in a running node (Settings → Theme → Layout): the shell re-laid-out without a
reload in every combination (band+sidebar, breadcrumbs+sidebar, band+topmenu, breadcrumbs+topmenu),
and the choice survived a refresh (prefs authority + localStorage cache). The band path is
byte-for-byte today's header; the rail is absent from the DOM in top-menu mode (asserted in design).

## Non-goals (recorded, not gaps)

- Breadcrumb depth beyond two levels (follow-up).
- Top-menu responsive collapse on narrow viewports (follow-up).
- A per-page sub-title registry for richer crumb trails (follow-up).

---

## Visual redo of the two renderers (2026-07-10 follow-up)

The chrome scope shipped green, but two renderers looked bad in practice. This follow-up is a
**visual-only** pass: identical props/contract/data, same shadcn primitives (`Menubar`,
`Breadcrumb`) — only the look changed. No axes, wiring, or test contracts touched; all 15 affected
tests stay green.

### 1. TopMenuNav (`ui/src/features/shell/TopMenuNav.tsx`)

**Before (why it read as a cramped toy strip):**
- The whole bar sat on `bg-card/60` with `py-1.5`, and each `Menubar` kept its default primitive
  chrome (`border rounded-md shadow-sm bg-panel h-9`). Result: **two bordered pills floating on a
  tinted strip** instead of one continuous app menubar.
- The account control was a lone `<LogOut>` glyph at the far right — read as a broken/empty element,
  not an account affordance.
- A leftover `NavMenuMotionItem` + `NavActivePill` wrapped the entire menubar, painting an accent
  box behind the whole strip — a rail concept that doesn't map to a horizontal menubar.
- No divider between brand and nav; no "you are here" cue once the sidebar is gone.

**After (VS Code / Linear top-bar altitude):**
- One **flat chrome bar** on `bg-panel-2` (the same raised chrome tone the rail uses) with a bottom
  hairline and `h-12`. The `Menubar`/`MenubarTrigger` box chrome is stripped via a `FLAT_MENUBAR`
  className override so triggers sit **flush** as quiet `text-muted` labels that light to `text-fg` +
  `bg-accent/10` on hover/open — a real desktop menubar, not floating pills.
- The menu that **owns the active surface** carries an accent **underline** (`ownsActive()` compares
  the active key against each entry's target) — the menubar's substitute for the rail's active pill.
- Brand block separated from the nav by a hairline divider (title-bar convention).
- A proper **account control**: a bordered ghost pill with an accent identity chip (`siteAbbr`
  initial on the accent→accent-2 gradient) + a chevron. `aria-label="Account"` preserved.
- Dropdown items got `gap-2.5 py-1.5`, muted icons, and an accent-tinted active row for scan clarity.

All chrome uses shell tokens only (`bg-panel-2/bg-card/border-border/text-fg/text-muted/accent/
accent-2`) — no hard-coded colors. Extension slots stay opaque `ext:<id>` refs (CLAUDE §10). The
removed `NavActivePill`/`NavMenuMotionItem` import is gone from this file (the rail still owns them).

### 2. HeaderBreadcrumbs (`ui/src/components/app/header-breadcrumbs.tsx`)

**Before:** a bare shadcn trail on `bg-background` with an undirected `border`, no anchor, and a
`text-muted-foreground` workspace crumb. Correct but characterless; the `icon` prop was unused.

**After:** shell tokens (`bg-bg border-border`); the surface `icon` now anchors the trail as a small
muted glyph (the page affordance the plain trail lacked, without the band header's accent chip); the
workspace crumb is a real link to `#/t/<ws>`; the current page reads `font-semibold tracking-tight`
for a touch more presence. Roles/labels the tests assert are unchanged.

### Verification

```
$ pnpm exec vitest run src/features/shell/TopMenuNav.test.tsx \
    src/components/app/header-breadcrumbs.test.tsx src/features/theme/LayoutTab.test.tsx
 ✓ src/components/app/header-breadcrumbs.test.tsx (4 tests)
 ✓ src/features/theme/LayoutTab.test.tsx (3 tests)
 ✓ src/features/shell/TopMenuNav.test.tsx (8 tests)
 Test Files  3 passed (3) | Tests  15 passed (15)

$ pnpm exec tsc --noEmit   # clean for both files
# (pre-existing PanelPage.tsx / createAppRouter.tsx errors from other sessions ignored)
```

### 3. Header pairing fix — top-menu forces the breadcrumb header (`ui/src/components/app/page.tsx`)

**The real clash the redesign exposed:** the Header axis (`band` | `breadcrumbs`) resolved
independently of the Nav axis, so `top menu + band` stacked the full-width menubar bar directly above
the **tall `band` header** (icon chip + description) — two heavy competing header bars back-to-back.
That was the "looks so bad" — not the menubar itself.

**Fix:** in `AppPage`, when `layout.nav === "topmenu"` the header always resolves to `breadcrumbs`
regardless of the header axis — the menubar + slim breadcrumb pairing (VS Code / Linear). The `band`
header now only pairs with the sidebar. The header axis still fully controls the sidebar layout; no
axis was removed. One-line derivation, no wiring/structure change in RoutedShell.

`tsc --noEmit` clean; the same 15 tests stay green (page.tsx resolution is exercised via the LayoutTab
+ renderer suites).

### 4. One continuous chrome unit in top-menu mode (menubar + breadcrumb merged)

Even after forcing breadcrumbs (§3), the menubar (bottom hairline) sat above the breadcrumb header
(`bg-bg`, its own top+bottom edges) — still read as two separate bars. Merged them into one block:

- `HeaderBreadcrumbs` gained a `seamless` prop (set only in top-menu mode). Seamless = share the
  menubar's `bg-panel-2` surface, height `h-12` (matches the menubar), and carry the **single**
  bottom hairline that closes the whole pair. Standalone (sidebar mode) is unchanged: `h-14`,
  `bg-bg`, its own border.
- `TopMenuNav` dropped its own `border-b`; it now grounds with a faint `shadow-[0_1px_0_0_border/0.4]`
  so there's a whisper-line between the two rows (they read as one grouped chrome block) and the bar
  still separates from content even on a rare surface that renders no header.

Result: menubar + breadcrumb are one continuous `--panel-2` chrome unit with a single closing
hairline — no more stacked-headers clash. `tsc` clean; the 15 tests stay green.

### 5. Floating menubar card + icons + right cluster (the "still looks bad" pass)

At full width the flat edge-to-edge strip read as unfinished: tiny text labels lost in a dark void,
no icons, a bare gear + lone account glyph floating top-right. Reworked into a real app menubar:

- **Floating card** (the horizontal peer of the floating-sidebar variant): the bar is now inset
  (`px-2 pt-2` on `bg-bg`) as a `rounded-lg border border-border bg-panel shadow-sm` card — a
  self-contained control surface, not a strip glued to the page edges. This also retires the §4
  "seamless merge" (a floating card can't be glued to the header) — the breadcrumb header returns to
  its standalone thin form under the card, and `HeaderBreadcrumbs` dropped the `seamless` prop.
- **Icon + label triggers.** Each built-in group bucket gets a leading icon via a top-menu-owned
  `GROUP_ICON` map keyed on the fixed group LABEL (Workspace→LayoutGrid, Automation→Workflow,
  Data→Database, Build→Boxes, System→Network) — presentation lives here, not in the shared
  `SURFACE_GROUPS`; it's core shell data, not an ext id, so rule 10 holds. Pinned/Extensions/Menu
  triggers already carry their glyphs. The active menu holds an accent tint + accent-colored icon.
- **Grouped right cluster.** Settings is a real icon button (`onSelect("settings")`) and the account
  menu an accent identity chip + chevron, both behind a hairline divider — no glyph floats alone.
- **Centered nav.** Brand pinned left, Settings+account pinned right, the trigger group centered in
  between (`flex-1 justify-center`) so a small 5-item set reads as intentional, not marooned.

`tsc` clean; the same 15 tests stay green (contracts are role/label/handler based — unaffected).
