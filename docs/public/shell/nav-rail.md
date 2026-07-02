# NavRail — the reusable sidebar package (`@nube/nav-rail`)

Shipped 2026-07-02. Source: [`packages/nav-rail/`](../../../packages/nav-rail/). Scope:
[`scope/frontend/nav-rail-scope.md`](../../scope/frontend/nav-rail-scope.md). Session:
[`sessions/shell/nav-rail-session.md`](../../sessions/shell/nav-rail-session.md).

A self-contained, data-driven sidebar library — the generic descendant of the lb shell's
`NavRail`, with shadcn/ui's Sidebar vendored as source. A workspace sibling of `ui/`, reusable by
other Nube apps. Two components, one data model.

## API

```ts
import { NavRail, NavMenu, type NavItem } from "@nube/nav-rail";
import "@nube/nav-rail/style.css";

interface NavItem { id: string; label: string; icon?: React.ComponentType; group?: string }
```

- **`NavRail`** — the app-shell collapsible **icon rail**. `SidebarProvider` + shadcn Sidebar;
  `collapsible="icon"`, tooltips when collapsed, `header`/`footer` slots, `⌘/Ctrl-B` toggle, mobile
  off-canvas. Props: `items`, `active`, `onSelect(id)`, `header?`, `footer?`, `defaultCollapsed?`,
  `className?`.
- **`NavMenu`** — an in-flow, **non-fixed** vertical menu (no provider, no `position:fixed`, no
  Sheet). For **section nav inside a panel/dialog**. Props: `items`, `active`, `onSelect(id)`,
  `badge?(id) → number|undefined`, `className?`, `aria-label?`.

Items sharing a `group` render under one label, in array order; ungrouped items render in a default
group. `onSelect` hands the id back — routing/content is the host's job. No app concepts
(`CoreSurface`, capabilities, `ThemeSwitcher`) — a host that gates entries filters `items` first.

## Theming

Self-themed like the ce-wiresheet editor: every color is `hsl(var(--nr-*))` scoped to a `.nav-rail`
root class (Tailwind v4 `@theme`, its own build). Dark by default; add `theme-light` to the root for
the light palette. A host re-skins by overriding the `--nr-*` vars at `:root`, via `className`, or
inline `style` — no fork. Ship the tokens with `import '@nube/nav-rail/style.css'`.

## In use

- **Dashboard `PanelEditor`** (`ui/src/features/dashboard/editor/PanelEditor.tsx`) — the Grafana-style
  panel editor's **options rail** is `NavMenu` (Query / Plot / Transform / Panel options / Field /
  Overrides), replacing the retired in-house `EditorTabs`. `NavMenu` (not `NavRail`) because the rail
  lives inside a Sheet, where the app-shell sidebar's fixed positioning would escape.

## Build & test

Own Vite lib build → `dist/nav-rail.{js,cjs}` + rolled-up `index.d.ts` + one bundled `nav-rail.css`;
React is a peer. `pnpm -C packages/nav-rail test` (12 real-component tests), `… typecheck`, `… build`.
Verified in the app against the **real** gateway (dashboard panel-editor suites).

## Deferred

- ce-wiresheet cross-repo consumer (via `file:` dist import).
- Migrating lb's own `ui/src/features/shell/NavRail.tsx` onto `@nube/nav-rail`.
