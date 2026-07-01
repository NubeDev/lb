# nav-rail — session log

Status: **done (2026-07-02)**. Scope: [`scope/frontend/nav-rail-scope.md`](../../scope/frontend/nav-rail-scope.md).

## Ask

Extract the liked lb shell sidebar (`ui/src/features/shell/NavRail.tsx`) into a **reusable,
data-driven library** — shadcn/ui Sidebar vendored as source, self-themed, generic. First use
was redirected mid-session by the user: **not** ce-wiresheet, but the lb dashboard **viz panel
editor** (`PanelEditor`) — replace its in-house options-rail tab strip with the reusable rail.

## What shipped

**Package `packages/nav-rail/` (`@nube/nav-rail`)** — a workspace sibling of `ui/`. The pnpm
workspace root was promoted from `ui/` to the repo root (`packages: ['ui', 'packages/*']`) so
`ui` can depend on it via `workspace:*`.

- **Two components, one data model** (`NavItem { id, label, icon?, group? }`):
  - `NavRail` — the app-shell collapsible icon rail. The faithful generic port of the lb
    original: `SidebarProvider` + vendored shadcn Sidebar, `collapsible="icon"`, tooltips when
    collapsed, header/footer slots, `⌘/Ctrl-B` toggle, mobile off-canvas Sheet. All lb concepts
    (`CoreSurface`, cap-gating, `ThemeSwitcher`) removed — the host passes `items`/`active`/
    `onSelect` + `header`/`footer` nodes.
  - `NavMenu` — an in-flow, non-fixed vertical menu (no provider, no `position:fixed`, no Sheet).
    For embedding as **section nav** inside a panel/dialog. Same items model + a `badge(id)` fn.
- **Vendored shadcn primitives** (`src/primitives/{sidebar,sheet,tooltip,button}.tsx`) + `cn` +
  `useIsMobile`, copied from `ui/src/components/ui/*` with two edits: relative imports (no `@/`
  alias) and lb's global tokens (`bg-panel`/`text-fg`/…) converted to the package's own
  `--nr-*` tokens. `sidebar.tsx` kept faithful (over the ≤400-line budget) and flagged as
  vendored; our files (`NavRail`, `NavMenu`, `items`) stay small.
- **Self-themed** like ce-wiresheet: Tailwind v4 `@theme` over `hsl(var(--nr-*))` tokens scoped
  to `.nav-rail`, dark default + `.theme-light`, host-overridable at `:root`/`className`/inline.
  Its own Vite lib build (ESM+CJS+dts + one bundled `style.css`); React is a peer.

**First use — dashboard `PanelEditor`** (`ui/src/features/dashboard/editor/PanelEditor.tsx`):
- Replaced the in-house `EditorTabs` (`Tabs.tsx`) options-rail strip with `NavMenu` as a left
  vertical section nav (`grid-cols-[9rem_1fr]`), items = Query / Plot / Transform / Panel options
  / Field / Overrides, `badge` = transform/override counts. `Tabs.tsx` **deleted** (no other
  importer). `ui/src/main.tsx` imports `@nube/nav-rail/style.css`.

## Tests (green)

- **Package unit** (`vitest` + `@testing-library/react`, real component, no fakes): 12 tests —
  `NavRail.test.tsx` (8: order/grouping, onSelect, aria-current, controlled host, header/footer
  slots, className/token override, `⌘/Ctrl-B` toggle, defaultCollapsed) + `NavMenu.test.tsx` (4:
  order/grouping, onSelect+active, badge, className). `pnpm typecheck` + `pnpm build` clean.
- **Integration (real gateway)**: `panelEditor.gateway.test.tsx` (6) + `flowsPanelEditor.gateway.test.tsx`
  (4) pass against the **real** spawned `test_gateway` node — the NavMenu swap works end to end.
- **No new type errors** in `ui`: baseline `tsc --noEmit` had 2 pre-existing errors in
  `FlowsCanvas.gateway.test.ts` (unrelated); after the change the count is unchanged.

## Debugging

**Symptom:** after `pnpm install` wired the workspace dep, `ui` `tsc --noEmit` flooded with
`'Check'/'GitPullRequest'/… cannot be used as a JSX component` (lucide-react) — a React-18 vs
`@types/react@19` collision. **Cause:** the new package declared React 19 + `@types/react@19` +
`lucide-react@0.453` dev/deps, hoisting a v19 type world into the shared store that leaked into
`ui`'s typecheck. **Fix:** aligned the package's dev React/types/lucide to `ui`'s (`react@^18.3.1`,
`@types/react@^18.3.12`, `lucide-react@^0.460.0`) so the workspace stays one type world. Logged to
[`debugging/frontend/react-types-19-collision.md`](../../debugging/frontend/react-types-19-collision.md).

## Follow-ups (deferred, not silent)

- **ce-wiresheet consumer** — the original first-use, redirected by the user to the dashboard. When
  taken up: depend via `file:` (separate repo) and import the built `dist` + `style.css`.
- **Migrate lb's own `ui/src/features/shell/NavRail.tsx` onto `@nube/nav-rail`** — pass
  `CoreSurface` items + cap filter + `ThemeSwitcher` as slots. Recommended, not done here.
- **Options search wiring** — `NavMenu` shows all sections; the PanelEditor's `OptionsSearch`
  still filters within a tab body, not across the rail (unchanged from before).
