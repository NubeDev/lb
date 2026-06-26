# Frontend scope

Status: draft scope.

Build a simple first UI for the Lazybones platform using the existing
`/home/user/code/rust/lazybones/ui` app as the visual reference. Keep the first
pass small: an operational shell, a dashboard, and a few static/detail surfaces
that prove the design system before the backend is fully wired.

## Reference

- Source look: `/home/user/code/rust/lazybones/ui`
- Start with:
  - `src/styles/globals.css`
  - `src/components/layout/sidebar.tsx`
  - `src/components/layout/topbar.tsx`
  - `src/components/ui/`
  - `src/features/dashboard/`
- If a clean copy is useful, copy or clone the reference into `/tmp` first, then
  port only the needed tokens, layout ideas, and component patterns into this
  repo. Do not edit the reference app while building this UI.

## P0 screens

- **Shell:** compact left rail, top bar, dark/light theme support, workspace
  switcher placeholder, connection status, and settings entry.
- **Dashboard:** node role, store, bus, extension runtime, MCP, sync, jobs, and
  capability health at a glance.
- **Extensions:** simple installed/available list with capability badges and
  placement labels.
- **Workspaces:** list of workspaces plus member/team/channel counts.
- **Settings:** API endpoint, node role, profile, and local/cloud connection
  placeholders.

## Visual direction

Use the same quiet control-surface feel as the reference app:

- near-black dark mode and warm paper light mode
- one warm amber accent
- hairline borders, low-contrast panels, and compact spacing
- lucide icons in icon buttons and navigation
- shadcn-style primitives over Tailwind tokens
- dense dashboard cards, not a marketing landing page

Avoid broad restyling, decorative gradients, oversized hero sections, and new
palette experiments. The first UI should feel like the current Lazybones app has
been adapted to the reusable core-stack product.

## Implementation notes

- Prefer React, TypeScript, Tailwind, and the same component shape used by the
  reference app unless the local repo already establishes something else.
- Copy component ideas selectively instead of importing the whole UI.
- Keep mock data local and obvious until real endpoints exist.
- Keep the navigation small; add deeper pages only when the backend surface
  exists.
- Extension UI slots can be placeholders in P0, but the shell should leave room
  for them.
