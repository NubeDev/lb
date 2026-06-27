# Frontend — URL routing (session)

- Date: 2026-06-27
- Scope: ../../scope/frontend/routing-scope.md
- Stage: S9/S10 frontend shell work on the shipped platform (STAGES.md)
- Status: in-progress

## Goal
Replace the shell's `useState` navigation with TanStack hash routing so every surface has a
shareable URL and page arguments are typed, defaulted search params. The acceptance gate is the
routing scope's test plan: real-gateway route rendering, search validation, cap-deny redirect,
workspace isolation, hash-history parity, and back/forward/reload behavior.

## What changed
**Put the workspace in the route as a guarded `/t/<ws>` prefix** (the user asked for `/t/<id>/…`).
Trigger: the user worried the tenant-less URLs (`#/channels?c=general`, `#/dashboards?from=…`) meant
the workspace wasn't being passed — a security concern. It wasn't a hole (workspace was always
derived from the verified token server-side; the URL only carried filters), but the user wanted the
workspace visible in the URL. The risk in doing that: a client-controllable workspace in the URL
*looks* like — and could become — the hole they feared. So the segment is a **hint + guard**, never
an authorization input.

- `surface.ts`: paths are now tenant-relative; added `tenantPath`, `fullPathForSurface`,
  `stripTenant`; `surfaceForPath` strips the `/t/<ws>` prefix first.
- `createAppRouter.tsx`: all surface routes nested under a new `/t/$ws` **layout route** whose
  `beforeLoad` compares `params.ws` to the verified `context.workspace` and `redirect`s a mismatch
  to the token's workspace (same surface + args). Bare/legacy tenant-less paths land on a
  `TenantlessRedirect` (root not-found + index) that grafts the token's workspace on, preserving the
  surface and search. `DefaultRedirect` (cap-deny) and nav builders now thread `/t/<ws>`.
- `RoutedShell.tsx`: nav/channel selection build full `/t/<ws>/…` targets via `fullPathForSurface`.

## Decisions & alternatives
- **Workspace as a *guarded path segment*, not a query hint or implicit-only.** Reverses the scope's
  earlier "workspace stays out of the URL" call. Rejected: (a) keeping it fully implicit — the user
  explicitly wants `/t/<id>/…`; (b) trusting the segment as the workspace — that *is* the §7 hole;
  (c) a `?ws=` query hint — a path segment reads cleaner and nests naturally under the layout route.
  The guard (`beforeLoad` rewrite) + the gateway's token-derived workspace are belt-and-suspenders.
- **Redirect via `useEffect`/imperative `navigate`, not a render-time `<Navigate to={dynamicHref}>`.**
  See Debugging — the dynamic-href `<Navigate>` re-evaluated every commit and tripped React's update
  cap. The cap-deny `DefaultRedirect` keeps a static typed `<Navigate>` (no loop there).

## Tests
Real gateway (`pnpm test:gateway`) — **83 passed, 0 render-loop warnings**; unit (`pnpm test`) —
27 passed; `tsc --noEmit` clean; lint 0 errors.
- **New (workspace-isolation, mandatory):** "rewrites a pasted link whose `/t/<ws>` targets another
  workspace to the recipient's own" — opens `#/t/victim-ws/channels?c=ops` under a `recipient-c`
  session, asserts the recipient's channel renders, the `Workspace recipient-c` switcher shows, and
  the URL is corrected to `#/t/recipient-c/channels?c=ops`. Proves the segment can't cross the wall.
- Updated the back/forward/reload and Share-link assertions to the `/t/<ws>/…` grammar.
- Existing cap-deny + "does not take workspace from a pasted URL" tests still green.

## Debugging
- **Infinite render loop / "Maximum update depth exceeded".** A render-time `<Navigate to={href}>`
  with a *dynamic* href (the tenant-less fallback) re-ran on every commit before history settled,
  spewing ~370 React warnings (tests still passed, but it's a real loop). Fix: fire the redirect once
  from `useEffect` via imperative `navigate({ to, replace })`, return `null`. Also the first attempt
  double-prefixed the hash (`#${tenantPath(...)}`) — `href`/`to` must be the bare path, hash history
  adds the `#`. Will log a `docs/debugging/frontend/` entry + regression note.

## Public / scope updates
- `scope/frontend/routing-scope.md`: rewrote the URL grammar to `/t/<ws>/…`, the tenancy/isolation
  bullet (guarded hint, not auth input), the example flow (acme→beta rewrite), and resolved the
  `?ws=` open question.

## Dead ends / surprises
- TanStack's typed `to`/`params`/`search` fought the *dynamic* surface in the fallback (the
  channels-shaped `{c}` search type leaked onto non-channels routes). Using a full `href` string for
  the dynamic fallback (and keeping typed `to` only where the surface is static) sidestepped it.

## Follow-ups
- Log the render-loop entry under `docs/debugging/frontend/` and update `docs/debugging/README.md`.
- STATUS.md update pending.
- Open question carried forward: offer an explicit "switch to <ws>?" prompt on a mismatched link
  instead of silently rewriting.
