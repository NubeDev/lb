# Session — the onboarding Setup wizard (frontend)

Date: 2026-07-09 · Scope: [`scope/nav/setup-wizard-scope.md`](../../scope/nav/setup-wizard-scope.md)

## Ask

From the admin/Access console: a **Setup / onboarding section** — a stepped wizard to (1) create a
user + team + assign the user, (2) give nav + dashboard access, (3) **preview** the access as a live
demo. Keep it simple; **nice UX**. Frontend-only was acceptable ("maybe just frontend for now").

## What shipped

A new **Setup** tab in `AdminView`, first after Overview. All frontend; **zero backend change** — the
wizard orchestrates already-shipped host verbs.

New files (`ui/src/features/admin/setup/`, one responsibility each per FILE-LAYOUT):

- `SetupWizard.tsx` — the four-step flow (Person → Team → Access → Preview) + footer nav + the
  per-step "advance" that runs the real verbs.
- `useSetup.ts` — the orchestration hook: loads the real sources (users/teams/roles/navs) and exposes
  `makeUser` / `makeTeam` / `joinTeam` / `grantRole` / `giveNavToTeam` / `makeNavDefault` /
  `effectiveCaps`, each a real verb.
- `Stepper.tsx` — the horizontal step rail (done / current / upcoming; jump back only).
- `PickOrCreate.tsx` — a reusable "existing / new" segmented control (used by the Person + Team steps).
- `AccessPreview.tsx` — the **live preview**: resolves the target user's effective caps
  (`authz.resolve`) and renders a faux-sidebar of the pages they'll see + a provenance summary.
- `previewReach.ts` — the pure caps→surfaces lens, **reusing `allowedSurfaces`** so the preview equals
  the real rail.

Wiring: `AdminView.tsx` gains the `setup` tab (shown for `user.manage` ∨ `teams.manage` ∨
`grants.assign`) and mounts `<SetupWizard>`.

## Key decisions (and rejected alternatives)

- **Grant to the team, not the user.** Access is assigned to `team:<id>` so every current/future member
  inherits it — the nav scope's "a role grants; a nav shapes" shape. *Rejected:* granting the role
  directly to the user (doesn't scale to a cohort; diverges from the team-centric model).
- **Preview reuses `allowedSurfaces`, doesn't re-implement it.** The preview is a lens over the *same*
  display gate the shell uses, so the two can never drift. *Rejected:* a bespoke "what pages" computation
  (a second source of truth = the exact bug the nav scope warns against).
- **No new backend.** Everything the wizard needs already exists as a gated verb. Keeps rules 5/10 for
  free. Frontend-only was explicitly acceptable.
- **Honest about reach.** The preview shows the cap-allowed page set (`authz.resolve` returns granted
  caps, not the login-folded `reach:*` a curated nav mints). That's the honest "pages this person can
  open" — documented in `previewReach.ts` so no one mistakes it for the post-login reach-narrowed rail.

## Tests

`SetupWizard.gateway.test.tsx` — real spawned gateway, no mocks (rule 9):

- end-to-end onboard (create → team → role+nav → preview shows the granted page + provenance; resolver
  confirms the inherited cap);
- **capability-deny** (only `nav.resolve` → no wizard + server refuses a raw `grants.assign`);
- **workspace-isolation** (user created in ws-A invisible in ws-B, resolves to no caps).

Typecheck clean (`tsc --noEmit`, 0 errors); eslint clean (two custom toggle/step `<button>`s carry the
project-standard `no-restricted-syntax` disable with justification, matching `SwitchControl.tsx`).

## Follow-ups / not done

- The Access step **picks** an existing nav; it doesn't author one inline (build in the Nav tab). A
  future pass could offer "quick nav" creation from the wizard.
- Preview shows surfaces; it could also render a specific nav's *resolved* menu for the user (needs a
  resolve-as-user verb — deferred, out of the frontend-only scope).
