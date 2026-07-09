# Session — login-page branding

Scope: `docs/scope/frontend/workspace-branding-scope.md` (the "login-page heading / logo" slice).

## Ask

Settings → Branding already brands the authenticated shell. Extend it to the **login page**:
an admin-set **logo**, **heading**, and **sub-heading**, persisted in the backend.

## What shipped

Rode the existing `ui_branding` prefs axis (opaque JSON blob on `workspace_prefs`, admin-owned via
`prefs.set_default`) — no new datastore, no new capability, no new gateway route. The blob is opaque
to Rust, so the three new fields persist byte-for-byte with **zero backend code change**.

- `lib/branding/branding-options.ts` — added `loginSubheading` + `loginLogoDataUri` to the `Branding`
  shape (`loginHeading` already existed as a stubbed field). Extended `normalizeBranding` (fail-closed
  per axis: string cap for the sub-heading, image-data-URI guard for the logo) and added
  `BRANDING_PLACEHOLDERS.loginHeading` / `.loginSubheading` (generic, never the product name).
- `features/settings/BrandingTab.tsx` — new **"Login page"** field group: Heading, Sub-heading, and a
  Login-logo upload (reuses `BrandImageField` + `readBrandImage`). Cap-gated read-only for non-admins,
  same as the rest of the tab.
- `features/session/LoginView.tsx` — the sign-in card now paints the entered workspace's brand from the
  workspace-keyed localStorage **boot cache** (`loadCachedBrand`), re-read live as the visitor edits the
  workspace field. Logo replaces the generic sign-in glyph; heading/sub-heading fall back to neutral
  defaults for a never-visited workspace.
- `lib/branding/index.ts` — export the cache helpers (`loadCachedBrand` etc.).

## Pre-auth design note

The login page runs with **no token**, so `prefs.resolve` is unavailable. The full scope's
`GET /public/branding/{ws}` public read route remains the deferred slice (it's a deliberate
workspace-wall break flagged for `/security-review`). This slice instead reuses the **already-shipped
workspace-keyed boot cache**: the brand a member resolved on any prior authenticated visit is cached in
localStorage and repainted pre-auth on the next sign-in. Persistence is real (backend `ui_branding`);
the cache is only the first-paint surface. A first-ever visitor to an unbranded/never-cached workspace
sees the neutral default — no product name, no flash.

## Tests (green)

- `lib/branding/branding-options.test.ts` — sub-heading kept; login-logo data-URI kept, non-image
  dropped. (12 passed)
- `features/session/LoginView.branding.test.tsx` — neutral default with no cache; cached brand painted;
  live re-brand on workspace edit. (3 passed)
- `cargo test -p lb-prefs --test ui_branding_test` — opaque blob round-trips unchanged + workspace
  isolation. (6 passed) — confirms the new fields persist with no schema change.
- `pnpm exec tsc --noEmit` clean.

## Follow-ups

- The pre-auth **public read route** (subdomain→workspace, whitelist-only) is still the open slice for a
  first-ever visitor to see a brand before any member has cached it. See the scope's open questions.
