# nav-rail scope — a reusable, data-driven sidebar package

Status: **shipped (2026-07-02)** — package `packages/nav-rail/` (`@nube/nav-rail`) built + green
(12 unit tests), first-used in the dashboard `PanelEditor` (the `NavMenu` options rail, replacing the
in-house `EditorTabs`); verified against the real gateway (panel-editor + flows-panel-editor gateway
suites pass). Promotes to `public/shell/nav-rail.md`. Session:
[`nav-rail`](../../sessions/shell/nav-rail-session.md).

> **Build note (as shipped):** the package exposes **two** components off one data model
> (`NavItem[]`): `NavRail` (the app-shell collapsible icon rail — the faithful generic port of the lb
> `NavRail`) and `NavMenu` (an in-flow, non-fixed vertical menu for embedding as **section nav** inside
> a panel/dialog). The dashboard first-use consumes `NavMenu` because the PanelEditor options rail lives
> inside a Sheet, where the app-shell sidebar's `position:fixed` would escape the container. Cross-repo
> use in `ce-wiresheet` is deferred (the user redirected the first-use to the dashboard viz panel).

We want the **collapsible icon-rail sidebar** we already like from the Lazybones shell
(`ui/src/features/shell/NavRail.tsx`) as a **standalone, reusable package** — not left
inline. It ships as `packages/nav-rail/` (`@nube/nav-rail`), a workspace sibling of `ui/`,
with its own build. It is consumed **first** by the `ce-wiresheet` editor (a separate Nube
repo), and can later replace the inline `NavRail.tsx` inside `ui/` itself. Unlike that
original — which hard-codes Lazybones surfaces (`channels`, `flows`, …), cap-gating, and a
`ThemeSwitcher` — the package is **fully generic and data-driven**: the host passes an
`items[]` config plus `active`/`onSelect` and optional header/footer slots.

## Goals

- One reusable sidebar package, matching the look of the current `NavRail`
  (collapsible-to-icon, grouped items, tooltips when collapsed, header brand slot, footer
  slot, `⌘/Ctrl-B` toggle), that both `ui/` and external Nube apps can consume.
- **Data-driven, zero app concepts.** No `CoreSurface` union, no capabilities, no
  `ThemeSwitcher`. The host supplies `items` (`{ id, label, icon, group? }`), `active`,
  `onSelect(id)`, and optional `header`/`footer` React nodes.
- **shadcn/ui Sidebar as the engine, vendored as source** (shadcn is copy-in source, not an
  npm dep — the "popular sidebar" *is* shadcn's). The package owns the primitives it needs
  (`sidebar`, `sheet`, `tooltip`, `button`) + `cn` + `useIsMobile`, so a consumer installs
  ONE package, not a shadcn setup.
- **Self-themed & host-overridable.** All color comes from `hsl(var(--nr-*))` tokens scoped
  under a `.nav-rail` root class, overridable by the host at `:root` or inline — the same
  contract ce-wiresheet uses for its editor. Dark default + `.theme-light` variant.
- First use: mount the rail in `ce-wiresheet/src`, wired to the editor's **real** surfaces
  (Components, Scripts, Schedule, Diagnostics, Agent) — no fake data.

## Non-goals

- Not porting lb's cap-gating, `Surface`/`ext:` routing, or extension-slot federation
  (`ui-federation-scope`) — the host decides what `items` to show (it can pre-filter by its
  own caps before passing them in).
- Not a router. `onSelect` hands the id back; routing/content is the host's job.
- Not a theme-switcher UI — that's a host footer slot.
- **Migrating `ui/`'s own `NavRail.tsx` onto the package is a follow-up, not this ship.**
  `ui/` keeps its file; this change only adds the package and wires the ce-wiresheet
  consumer. (See Open questions.)

## Intent / approach

Two layers, matching how shadcn is meant to be used:

1. **Primitives (vendored shadcn):** `src/primitives/{sidebar,sheet,tooltip,button}.tsx`
   plus `src/lib/cn.ts` and `src/hooks/use-mobile.ts`. Copied faithfully from
   `ui/src/components/ui/*`, with two edits: imports rewritten off the `@/` alias to
   relative paths, and color utilities pointed at the package's own `--nr-*` tokens instead
   of lb's global `bg-bg`/`text-fg`. Internal — the package does not re-export the raw
   shadcn API as its contract, so the engine stays swappable.
2. **`NavRail` (our component):** the generic, data-driven wrapper — the reusable thing.
   `SidebarHeader` (header slot) → grouped `SidebarMenu` from `items` → `SidebarFooter`
   (footer slot), with `collapsible="icon"`, tooltips, and `aria-current`. ~120 lines like
   the original, minus the lb-isms.

Build mirrors ce-wiresheet's lib setup: Tailwind v4 `@theme` over `hsl(var())` tokens
scoped to `.nav-rail`, Vite lib build (ESM+CJS+dts), React as a peer dep, one bundled
stylesheet (`@nube/nav-rail/style.css`). The package builds its **own** CSS, so the fact
that `ui/` is on Tailwind v3 and the package is on v4 doesn't cross the boundary — the
consumer imports the prebuilt stylesheet.

**Placement — repo-root workspace.** The pnpm workspace was rooted at `ui/`; we promote it
to the repo root (`packages: ['ui', 'packages/*']`) so `ui/` can depend on the sibling via
`workspace:*` without a publish/link dance.

**Alternative rejected — leave NavRail inline / copy it into the consumer:** faster, but
carries lb's `CoreSurface`/cap/ThemeSwitcher coupling and gives no reuse. A workspace
package is the reuse boundary the user asked for ("make sure it's a library").

**Alternative rejected — a foreign npm sidebar (`react-pro-sidebar`, MUI Drawer):** none
match the NavRail look or the scoped-token theming without heavy restyling, and they drag in
a competing style system. The "popular" sidebar *is* shadcn's Sidebar — source-vendored by
design, which is what we do.

## How it fits the core

`nav-rail` is a **presentational frontend library**, not a node — the platform principles
about nodes are N/A, stated explicitly so the build doesn't invent them:

- **Tenancy / caps / SurrealDB / Zenoh / MCP / sync / secrets:** **N/A** — no backend, no
  workspace, no verbs. A host that needs cap-gating filters `items` before passing them.
- **One responsibility per file** (FILE-LAYOUT — applies to `.tsx`): `NavRail.tsx` (render),
  `items.ts` (the `NavItem`/`NavGroup` types), one primitive per file, `use-mobile.ts`,
  `cn.ts`. No `utils.ts`. The vendored `sidebar.tsx` is faithful to shadcn and runs over the
  ≤400-line budget — kept intact (forking shadcn to split it invites drift); flagged as
  vendored so it isn't read as our code. OUR files stay small.
- **No mocks / no fake backend** (CLAUDE §9): the rail is presentational — no backend to
  fake. Tests render the **real** component with real `items`; the ce-wiresheet integration
  wires it to the editor's **real** surfaces, not a stub list. No `*.fake.ts`.
- **Symmetric / stateless:** the component holds only view state (open/collapsed); no
  durable state. N/A to node symmetry.
- **Stable public surface:** exports `NavRail`, the `NavItem`/`NavGroup` types, and
  `./style.css`. Vendored primitives stay internal.

## MCP surface / API shape

A presentational component — **no CRUD / get-list / watch / batch** (those are node
concepts, N/A). The surface is one component + its data model:

```ts
export interface NavItem { id: string; label: string; icon?: React.ComponentType; group?: string }
export interface NavRailProps {
  items: NavItem[];
  active: string | null;
  onSelect: (id: string) => void;
  header?: React.ReactNode;      // brand/logo slot (collapses to an icon)
  footer?: React.ReactNode;      // e.g. host's theme switcher / sign-out
  defaultCollapsed?: boolean;
  className?: string;            // extra classes on the root (host theming hook)
}
export function NavRail(props: NavRailProps): JSX.Element
```

Items with the same `group` render under one `SidebarGroupLabel`; ungrouped items render in
a default group. Order is array order.

## Example flow

1. A host renders `<NavRail items={[{id:'components',label:'Components',icon:Boxes},
   {id:'scripts',label:'Scripts',icon:Braces,group:'Author'}]} active={sel}
   onSelect={setSel} header={<Brand/>} />`.
2. Collapsed (icon) mode shows only icons with hover tooltips; `⌘/Ctrl-B` toggles.
3. Clicking "Scripts" calls `onSelect('scripts')`; the host swaps its content pane. The rail
   marks it `aria-current="page"`.
4. ce-wiresheet mounts this rail as its left chrome, `items` = the editor's real surfaces,
   `onSelect` drives the existing tab/panel host (`UiTabHost`/`TabShell`).

## Testing plan

`packages/nav-rail` uses `vitest` + `@testing-library/react` (jsdom). Mandatory categories
from `scope/testing/testing-scope.md` mapped to a presentational lib:

- **Real component, no fakes** (the §0 rule that applies): tests render the **real**
  `NavRail` with real `items`; no `*.fake` re-implementation.
- **Unit / interaction:**
  - one button per item, grouped by `group`, in array order;
  - clicking an item calls `onSelect` with its id; the active item has
    `aria-current="page"`;
  - collapsed mode hides labels / exposes tooltips; `⌘/Ctrl-B` toggles state;
  - header/footer slots render when provided, absent when not;
  - a host `className` / inline `--nr-*` override reaches the root (theming contract).
- **Integration (ce-wiresheet):** mount the editor chrome with the rail wired to real
  surfaces; assert selecting a surface changes the active panel — against real editor
  components, not a stub.

**Capability-deny and workspace-isolation tests are N/A** — no caps or tenancy in a
presentational frontend lib. Stated so the implementing session doesn't invent them.

## Risks & hard problems

- **Tailwind `@theme` scoping.** Color utilities only resolve inside the lib's own `@theme`
  build (ce-wiresheet learned this the hard way). The package ships its own `@theme` +
  tokens; without it `bg-nr-panel` won't resolve in a host build. Mirror `wiresheet.css`.
- **Two token vocabularies.** The original uses `bg-panel`/`text-fg`; the package uses
  `--nr-*`. The vendored copy must be fully converted or the rail renders mis-styled.
  Covered by the theming test.
- **Faithful vendor vs. the ≤400-line rule.** `sidebar.tsx` kept verbatim breaks the budget;
  splitting risks upstream drift. Decision: keep faithful + isolated, keep OUR files small,
  flag it.
- **Peer-dep split.** Radix + cva are `dependencies`; React is a peer + external in the lib
  build — a wrong split double-loads React in a host.
- **Workspace promotion.** Moving the pnpm root from `ui/` to the repo root must not break
  `ui/`'s existing install/build. Verify `pnpm install` + `ui` build/test stay green.

## Open questions

1. **Migrate `ui/`'s `NavRail.tsx` onto the package?** Recommended (lb passes its
   `CoreSurface` items + cap filter + `ThemeSwitcher` as slots), but out of scope here.
2. **npm scope / publish:** `@nube/nav-rail` assumed, consumed via `workspace:*` (lb) and
   file/git (ce-wiresheet) for now. Confirm before publishing.
3. **Mobile off-canvas:** the Sheet-based mobile drawer comes free via the vendored
   primitives. Keep it (default) or desktop-icon only?
4. **In ce-wiresheet, does the rail REPLACE the `ExtensionStrip`/`TabShell` chrome or sit
   alongside as top-level nav?** Default: top-level nav alongside. Affects integration
   wiring, not the package.

## Related

- Design source: `ui/src/features/shell/NavRail.tsx` (the liked original) and
  `ui/src/components/ui/sidebar.tsx` (the shadcn engine vendored).
- Consumer repo: `ce-wiresheet/src` — theming pattern mirrored from its `wiresheet.css` /
  `wiresheet-theme.css`; sits beside `ExtensionStrip.tsx` / `TabShell.tsx`.
- Sibling scope: `scope/extensions/ui-federation-scope.md` (extension sidebar slots — NOT
  ported here; the host owns `items`).
- No SKILL.md: presentational UI component, no agent-/API-drivable surface (no MCP verbs, no
  gateway routes). **N/A.**
