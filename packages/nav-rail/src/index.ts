// `@nube/nav-rail` — public surface.
//
// A reusable, data-driven sidebar. Pass `items` + `active`/`onSelect` and optional
// header/footer slots; the rail renders a collapsible icon-rail (shadcn/ui Sidebar,
// vendored internally). Self-themed via `hsl(var(--nr-*))` tokens scoped to `.nav-rail`,
// host-overridable. Everything under `primitives/` is internal.
//
// Consumed two ways:
//   - `workspace:*` from the lb `ui/` app,
//   - imported by an external host (e.g. ce-wiresheet) — `import { NavRail }` plus
//     `import '@nube/nav-rail/style.css'`.

import "./nav-rail.css"; // bundles Tailwind + the rail's tokens into the lib stylesheet

export { NavRail } from "./NavRail";
export type { NavRailProps } from "./NavRail";
export type { NavItem, NavGroup } from "./items";
