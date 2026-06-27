# Frontend — Extensions shadcn details (session)

- Date: 2026-06-28
- Scope: ../../scope/frontend/ui-standards-scope.md
- Stage: UI standards migration
- Status: done

## Goal

Migrate the Extensions console off the legacy control layer and show the installed extension's UI
support details, including page entry, bridge scope, widget declarations, runtime state, and restarts.

## What changed

- `ui/src/features/extensions/ExtensionsView.tsx` now uses `AppPageHeader`, `Button`, `Badge`, and
  `Card`, and expands each install into a dense support panel.
- The extension support section now scales for many pages/widgets and many verbs: surfaces are counted
  and previewed with an expandable list, and bridge verbs are hidden behind an inspect/search panel
  capped to 100 rendered results at a time.
- `ui/src/features/extensions/UploadArtifact.tsx` now uses the local `Button` and `Input` primitives.
- `ui/src/components/ui/button.tsx` gained the destructive variant expected by the UI standard.
- `ui/src/components/ui/input.tsx` now forwards refs, which keeps hidden file inputs usable.
- `ui/eslint.config.js` removed the Extensions files from `LEGACY_VIEWS`.

## Decisions & alternatives

- Kept the page as a list of repeated extension cards, with divided detail sections inside each card,
  instead of nesting cards inside cards.
- Rendered `ExtRow.ui` and `ExtRow.widgets` from the real API shape instead of hardcoding the
  `proof-panel` JSON.

## Tests

- Initial migration pass:
  - `pnpm lint` — 0 errors, 141 warnings in remaining legacy allowlist files.
  - `pnpm build` — `tsc --noEmit && vite build` passed.
  - `pnpm test:gateway src/features/extensions/ExtensionsView.gateway.test.tsx` — 4 passed.
- Follow-up scalability refinement:
  - `pnpm exec eslint src/features/extensions/ExtensionsView.tsx src/features/extensions/UploadArtifact.tsx`
    — passed.
  - Gateway rerun was blocked by a concurrent `cargo test --workspace` holding the Rust build lock.

The gateway test requires local TCP binding; the first sandboxed run failed before tests because
Zenoh could not bind a listener. The approved rerun passed.

## Debugging

None opened. The migration surfaced two test-time issues and fixed them directly: `Input` needed
`forwardRef`, and the richer row duplicated the exact `disabled` health text the lifecycle test
expects to be unique.

## Public / scope updates

- Updated `../../scope/frontend/ui-standards-scope.md` with the new lint warning and legacy-view count.
- Updated `../../public/frontend/frontend.md` to list Extensions as a migrated reference surface.

## Dead ends / surprises

The local dev server from the screenshot was not running on port 5173 during verification.

## Follow-ups

- Migrate `ConfirmDestructive` to shadcn `Dialog`, `Button`, `Input`, and checkbox/switch primitives.
- Continue shrinking `LEGACY_VIEWS` one surface at a time.
