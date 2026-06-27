# Frontend — UI standards (shadcn-first, consistent, responsive) + ESLint enforcement (session)

- Date: 2026-06-27
- Scope: ../../scope/frontend/ui-standards-scope.md
- Stage: post-S8 frontend hardening (cross-cutting UI standard). See STAGES.md / STATUS.md.
- Status: done (scope + enforcement framework landed; the 27-view migration is the follow-on backlog)

## Goal
Codify the UI conventions the user settled on the Members page + NavRail sidebar — **shadcn/ui
primitives only, one consistent look, mobile auto-resize** — into an authoritative scope doc, and
make the "use shadcn, not the legacy control layer" rule **mechanically enforced** rather than a
prose hope. The breakdown being closed: shadcn was intended from day one but never wired in; the
shell grew a parallel control layer in `globals.css` (`.control-field`, `.soft-button`,
`.page-header`, …) that 37/40 feature views use. Members + NavRail are the only conforming surfaces.

## What changed
- `docs/scope/frontend/ui-standards-scope.md` — **new** authoritative scope: the three rules, an
  enforceable done-checklist, the component backlog (`select`/`dialog`/`table`/`switch`/`alert`/
  `tabs`/`card` to generate), the incremental migration plan, and the testing plan (real gateway
  harness — no fakes, CLAUDE §9).
- `docs/scope/frontend/ui-design-scope.md`, `docs/scope/README.md`, `docs/public/frontend/frontend.md`
  — cross-linked the standard in (look-doc → how-doc, scope index, as-built pointer).
- `ui/eslint.config.js` — **new** flat config. Bans raw `<button>`/`<input>`/`<select>`/`<textarea>`
  and the legacy `globals.css` control classes in `src/**` (the `components/ui/*` primitives are
  exempt — they legitimately wrap raw elements). Adds `react-hooks` recommended rules (the source
  already carried `eslint-disable react-hooks/*` directives expecting the plugin). Enforcement is
  **forward-only**: the rule is an *error* everywhere, downgraded to *warning* for the 27 files in
  the `LEGACY_VIEWS` allowlist. Migrate a view → delete its path from `LEGACY_VIEWS` → it errors on
  regression. When the list empties, flip `lint` to `--max-warnings 0`.
- `ui/package.json` — added `"lint": "eslint src"`; dev-deps `eslint@9`, `typescript-eslint@8`,
  `eslint-plugin-react@7`, `eslint-plugin-react-hooks@5`.

## Testing / verification (shown green)
- `pnpm lint` → **exit 0**, `153 problems (0 errors, 153 warnings)` across the 27 legacy views —
  the freeze lands without a wall of errors; the backlog is visible and counted.
- `eslint src/features/members/MembersView.tsx src/features/shell/NavRail.tsx` → **clean (exit 0)** —
  confirms the canonical reference is 100% on the primitives.
- Forward-only gate proven: a throwaway `<button className="soft-button">` in `features/members/`
  (a conforming, non-allowlisted area) → **2 errors, exit 1**. Probe file removed after.
- react-hooks directives now resolve (no more "rule not found" errors on `useDashboard.ts`/`useSeries.ts`).

## Notes / follow-ons (not regressions)
- The 153 warnings are the migration backlog from `ui-standards-scope.md` "Migration", not bugs.
- Open: wire `pnpm lint` into CI / the gateway-test job so the freeze runs on every change.
- Open: generate the missing shadcn primitives (component backlog) ahead of the per-area migration,
  starting with the admin tables (worst offender for both shadcn and responsive).
- No debug entry: nothing broke; this was additive (new doc + new lint config + dev-deps).
