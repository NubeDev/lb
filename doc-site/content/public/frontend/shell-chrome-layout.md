---
title: Shell chrome — header style & top-nav mode
description: Two appearance axes for the app shell chrome — a breadcrumb header style and a top-menu nav mode — on the Layout tab.
---

# Shell chrome — header style & top-nav mode

Two **appearance choices** for the app shell chrome, set from **Settings → Theme → Layout** and
persisted per-member through the same `ui_theme` prefs blob as every other Layout axis. Both are pure
shell chrome, additive, and ride the existing `prefs.set` / `prefs.set_default` / `prefs.resolve`
verbs — **no new verb, cap, table, or MCP surface**.

## Header style — `band` | `breadcrumbs`

- **Band** (default) — today's `AppPageHeader` icon-chip band. Pixel-identical, unchanged.
- **Breadcrumbs** — a clean **shadcn/ui `Breadcrumb`** header rendering
  `Workspace / <Surface>`, the way shadcn renders breadcrumbs (no icon chip, no gradient). The
  top-right actions slot (workspace chip + Settings gear) is preserved.

## Navigation mode — `sidebar` | `topmenu`

- **Sidebar** (default) — today's left `NavRail`. Unchanged.
- **Top menu** — a horizontal **shadcn/ui `Menubar`** mounted above the content (the left rail is
  omitted entirely). Each workspace nav bucket (`Workspace`, `Automation`, `Data`, `Build`, `System`)
  becomes a `MenubarMenu`; its surfaces become dropdown items. A resolved/curated nav renders the same
  way. **Pinned** favorites and **Extensions** get their own menus when non-empty; the no-lockout
  escape hatch (**Show all pages** / **Use my menu**) and **Sign out** live in a right-aligned account
  menu. Extension ids stay opaque `ext:<id>` refs — no branch on identity.

The top menu is a **second renderer** over the exact same resolved-nav data the rail consumes
(`ResolvedNavItem[]`, `SURFACE_GROUPS`, pins, ext slots) — not a new source of truth.

## Two-axis interaction

When `nav === "topmenu"`, the sidebar-specific controls (Variant / Collapsible / Position) are still
writable but visibly marked **"sidebar only"** — they no-op on the layout while the top menu is
active, but their values are kept (never cleared). Switching back to `sidebar` restores them intact.

## Persistence & authority

Both axes live inside the opaque `ui_theme` prefs blob on the existing member/workspace prefs record.
An admin sets the workspace default via the tab's existing "Set as workspace default" action
(`prefs.set_default`); a member overrides per-device via `prefs.set`. The choice roams to every
device and folds member → workspace-default → built-in exactly as the existing layout fields do.
A member lacking `prefs.set` degrades to local-only (cache), no crash — the existing prefs deny path.

## Scope & session

- Scope: [`docs/scope/frontend/shell-chrome-layout-scope.md`](../../../../docs/scope/frontend/shell-chrome-layout-scope.md)
- Session: [`docs/sessions/frontend/shell-chrome-layout-session.md`](../../../../docs/sessions/frontend/shell-chrome-layout-session.md)
