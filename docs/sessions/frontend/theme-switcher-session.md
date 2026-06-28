# Frontend — theme switcher (session)

- Date: 2026-06-28
- Scope: ../../scope/frontend/theme-switcher-scope.md
- Stage: S9/S10 frontend shell hardening
- Status: done

## Goal

Build the scoped theme switcher end to end in the React shell: reusable theme infrastructure, shadcn-style controls, three accent palettes, light/dark mode, tests, and shipped docs.

## What changed

- Added `ui/src/lib/theme/`: theme options, validation, storage fallback, DOM application,
  `ThemeProvider`, and `useTheme`.
- Added `ui/src/features/theme/ThemeSwitcher.tsx` and mounted it in `features/shell/NavRail.tsx`.
  The control uses shadcn-style `Button`/`Tooltip` primitives, lucide light/dark icons, and
  token-bound swatches for amber, teal, and blue.
- Updated `ui/src/styles/globals.css` with three accent palettes across light/dark mode while keeping
  the existing amber identity as the default.
- Added a guarded `index.html` bootstrap script so a saved theme is applied before React mounts.
- Hardened shadcn primitives by forwarding refs through `Button` and `Sheet` wrappers, removing Radix
  composition warnings.
- Removed a stale unused `ExtUi` type import in `ExtensionsView.tsx` that blocked `pnpm build`.

## Decisions & alternatives

- Chose a local browser/webview preference (`localStorage` key `lb.theme`) over a backend preference
  record. This slice has no workspace data, capability gate, or MCP surface; a synced setting belongs
  in the separate `prefs/` topic.
- Chose explicit `dark` / `light` modes only, matching the request. A system-following mode remains a
  future product choice.
- Chose amber, teal, and blue. Amber preserves the existing identity; teal and blue are distinct enough
  to be useful without making the operator shell loud. Light amber was darkened to clear AA contrast.
- Rejected a one-off `NavRail` `useState` toggle because it would scatter DOM/storage details across
  the shell instead of leaving a reusable theme boundary.

## Tests

- `pnpm test` — 13 files / 56 tests passed.
- `pnpm test:gateway` — 25 files / 110 tests passed against the real spawned gateway node.
- `pnpm build` — `tsc --noEmit && vite build` passed. Vite still reports the existing large-chunk
  warning for the app bundle.
- `pnpm lint` — 0 errors, 141 existing forward-migration warnings from legacy UI-standard allowlist
  files.
- Palette contrast check — accent text against base background: light amber 4.66:1, light teal 4.80:1,
  light blue 6.38:1, dark amber 8.98:1, dark teal 10.13:1, dark blue 6.46:1.

## Debugging

None opened. The build initially exposed an obvious stale type import (`ExtUi`) in `ExtensionsView.tsx`;
it was removed directly and covered by the successful rebuild. One `test:gateway` run hit a transient
channel-list wait in `App.gateway.test.tsx`; the immediate rerun passed 110/110.

## Public / scope updates

- Updated `docs/scope/frontend/theme-switcher-scope.md` with resolved palette decisions.
- Updated `docs/public/frontend/frontend.md` with the shipped theme preference behavior.
- Updated `docs/public/SCOPE.md` and `docs/STATUS.md` with the shipped frontend slice.

## Dead ends / surprises

- `Tooltip` requires a provider when `ThemeSwitcher` is rendered outside `SidebarProvider`; the switcher
  now owns a `TooltipProvider`, so it is reusable in tests or future placements.
- The local `Button` and `Sheet` wrappers did not forward refs, which Radix composition expects. Both
  now forward refs like standard shadcn primitives.

## Follow-ups

- Consider a system-following mode only if product usage asks for it.
- Promote local theme preferences into the `prefs/` backend topic only if users need roaming settings.
