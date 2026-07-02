# @nube/nav-rail

A reusable, data-driven **sidebar** for React. The generic descendant of the Lazybones shell's
`NavRail`, with [shadcn/ui](https://ui.shadcn.com)'s Sidebar vendored as source — self-themed,
host-overridable, zero app concepts. Two components, one data model.

```tsx
import { NavRail, NavMenu, type NavItem } from "@nube/nav-rail";
import "@nube/nav-rail/style.css";
import { Boxes, Braces } from "lucide-react";

const items: NavItem[] = [
  { id: "components", label: "Components", icon: Boxes },
  { id: "scripts", label: "Scripts", icon: Braces, group: "Author" },
];

// App-shell collapsible icon rail
<NavRail items={items} active={sel} onSelect={setSel} header={<Brand />} />

// Embedded section nav (inside a panel/dialog) — no fixed positioning
<NavMenu items={items} active={sel} onSelect={setSel} badge={(id) => counts[id]} />
```

## Components

| | shape | use for |
|---|---|---|
| **`NavRail`** | app-shell sidebar (`SidebarProvider`, `collapsible="icon"`, tooltips, `⌘/Ctrl-B`, mobile off-canvas, header/footer slots) | the left edge of an app |
| **`NavMenu`** | in-flow vertical menu (no provider, no `position:fixed`, no Sheet) | section nav **inside** a panel/dialog |

Both take `items: NavItem[]` (`{ id, label, icon?, group? }`), `active`, `onSelect(id)`. Items sharing
a `group` render under one label, in array order. `onSelect` hands the id back — routing is the host's
job. No `CoreSurface`/capabilities/theme-switcher baked in; gate `items` before passing them.

## Theming

Every color is `hsl(var(--nr-*))` scoped to a `.nav-rail` root class (Tailwind v4 `@theme`, built into
the shipped stylesheet). Dark by default; add `className="theme-light"` for the light palette. Re-skin
by overriding `--nr-*` at `:root`, via `className`, or inline `style` — no fork.

## Develop

```
pnpm test        # 12 real-component tests (vitest + @testing-library/react)
pnpm typecheck
pnpm build       # dist/nav-rail.{js,cjs} + index.d.ts + nav-rail.css
```

React (`react`, `react-dom`) is a **peer** dep — the host provides one copy. Consumed via `workspace:*`
inside this repo; external hosts import the built `dist` + `@nube/nav-rail/style.css`.

## Layout

- `src/NavRail.tsx`, `src/NavMenu.tsx` — the two public components.
- `src/items.ts` — the `NavItem`/`NavGroup` model + `groupItems`.
- `src/primitives/` — **vendored** shadcn (`sidebar`, `sheet`, `tooltip`, `button`); internal, kept
  faithful to upstream. `src/lib/cn.ts`, `src/hooks/use-mobile.ts`.
- `src/nav-rail.css` / `src/nav-rail-theme.css` — the `@theme` + scoped `--nr-*` tokens.
