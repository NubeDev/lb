# Frontend scope — theme switcher

Status: shipped (2026-06-28). Promoted to `public/frontend/frontend.md`.

Add a maintained theme preference layer to the React shell: users can choose light or dark mode and one of three accent palettes. The feature should feel like the rest of the shadcn-first operator UI, preserve the current amber identity as the default, and leave reusable theme infrastructure future screens can consume without reaching into `document` or `localStorage` directly.

## Goals

- Ship a shell-level theme switcher for **light / dark** mode and **three accent palettes**.
- Keep theme state local to the browser/webview for now, applied before the user has to navigate.
- Centralize theme options, validation, DOM application, and persistence under one reusable frontend module.
- Express palettes through existing CSS variable tokens so shadcn primitives, graphs, dashboards, and extension host surfaces inherit the result without component-specific color branches.
- Add focused tests for preference validation, persistence fallback, DOM class/attribute application, and the visible switcher control.

## Non-goals

- No backend preference API, MCP tool, SurrealDB record, or workspace-synced preference in this slice.
- No redesign of page layouts, navigation, typography, or component primitives.
- No full system-theme mode yet; the user asked for light/dark, so this ships explicit choices only.
- No per-extension theme contract change. Federated pages still inherit host CSS variables as they do today.

## Intent / approach

Treat theming as a small frontend domain, not as ad hoc shell code. A `lib/theme` module owns the canonical theme options, storage key, storage validation, document application, context provider, and hook. The switcher component consumes only that hook and shadcn primitives. CSS variables remain the single contract between Tailwind, shadcn components, first-party screens, graphs, and extension UI.

The rejected alternative is a one-off `useState` inside `NavRail` that toggles `document.documentElement.classList`. That would work once, but every future preference surface would need to rediscover storage keys, valid values, SSR/test guards, and DOM attributes. Centralizing the layer keeps the long-term maintenance boundary small and searchable.

## How it fits the core

- **Tenancy / isolation:** N/A for this slice. The preference is browser/webview local and does not read or write workspace data.
- **Capabilities:** N/A. Theme changes are local presentation changes, not privileged platform actions.
- **Placement:** either. The same React shell runs in browser and Tauri; storage uses `window.localStorage` when available and falls back to defaults when unavailable.
- **MCP surface:** none. No CRUD, list, watch, or batch verbs are introduced.
- **Data (SurrealDB):** none. A future synced preference layer can promote this into the existing `prefs/` topic; this slice intentionally avoids a half-built backend surface.
- **Bus (Zenoh):** none. Theme changes are local state, not motion.
- **Sync / authority:** node-local/browser-local for now. There is no offline merge rule because no node data is written.
- **Secrets:** none.
- **One responsibility per file:** split storage, DOM application, options, provider, hook, and switcher UI into named files under `ui/src/lib/theme` and `ui/src/features/theme`.
- **SDK/WIT impact:** none. The plugin boundary remains unchanged.

## Example flow

1. A user signs in and the shell renders.
2. `ThemeProvider` loads a validated preference from local storage, or defaults to dark + amber.
3. The provider applies `class="dark"` or no dark class plus `data-theme-accent="<accent>"` to `<html>`.
4. The user opens the shell footer theme controls, chooses light mode and the teal accent.
5. CSS variables update immediately; shadcn buttons, sidebar state, graphs, and panels inherit the new tokens.
6. The preference is saved locally and restored on the next reload.

## Testing plan

From `scope/testing/testing-scope.md`, the mandatory platform data categories do not apply because this slice has no backend verb, record, bus subject, or synced state.

- **Frontend unit/component tests:** verify storage validation and fallback, document class/attribute application, provider persistence, and switcher interaction.
- **Accessibility smoke:** switcher controls expose labels, `aria-pressed`, and visible focus via the existing `Button` primitive.
- **No fake backend:** tests are pure frontend because the feature has no backend dependency; no mocked node or fake transport is introduced.
- **Build/lint:** run `pnpm build`, `pnpm lint`, and the focused/default Vitest suite.

## Risks & hard problems

- **Palette contrast drift:** every accent must remain readable in both light and dark mode. Keep body text tied to `fg`, not accent, and use saturated accents mainly for current selection, focus, and primary actions.
- **CSS token compatibility:** many existing screens use both custom tokens (`bg`, `panel`, `fg`) and shadcn tokens (`background`, `card`, `primary`). The theme layer must set both from one palette to avoid subtle mismatches.
- **Early paint flash:** the current `index.html` hardcodes `class="dark"`. Provider-driven application should preserve the dark default but remove the static assumption so the stored light preference is authoritative after app boot.
- **Control density in collapsed nav:** the switcher must remain usable in the icon rail without bloating the sidebar or hiding sign-out.

## Open questions

- **Exact palette names and values:** resolved — amber (default), teal, and blue. Accent text contrast
  against the base page background is AA-or-better in both light and dark mode: light amber 4.66:1,
  light teal 4.80:1, light blue 6.38:1, dark amber 8.98:1, dark teal 10.13:1, dark blue 6.46:1.
- **System mode:** deferred. Light/dark explicit choices are the contract for this slice.
- **Workspace-synced preferences:** deferred to the existing `prefs/` topic if users need settings to
  roam between machines.

## Related

- `ui-standards-scope.md` — shadcn-first control layer and token discipline.
- `ui-design-scope.md` — existing dark-first amber operator-console visual direction.
- `../../public/frontend/frontend.md` — current frontend as-built docs.
- `../../../docs/FILE-LAYOUT.md` §4 — frontend one-component / one-hook file boundaries.
