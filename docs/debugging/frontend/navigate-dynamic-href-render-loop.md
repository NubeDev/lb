# A render-time `<Navigate to={dynamicHref}>` loops ("Maximum update depth exceeded")

**Symptom (test stderr):** after nesting all routes under the new `/t/<ws>` tenant prefix,
`pnpm test:gateway` still went green (83 passed) but spewed ~370 React warnings:

```
Error: Maximum update depth exceeded. This can happen when a component repeatedly
calls setState inside componentWillUpdate or componentDidUpdate. React limits the
number of nested updates to prevent infinite loops.
  ❯ Router.Transitioner.router.startTransition .../Transitioner.tsx:27
```

Emitted from any test that started at a **tenant-less** hash (`#/dashboards`, `#/members`, …) and
fell through to the redirect that grafts the token's workspace on.

**Affected:** the `TenantlessRedirect` fallback (root not-found + index route) in
`ui/src/features/routing/createAppRouter.tsx`. The cap-deny `DefaultRedirect` did **not** loop.

## Root cause

`TenantlessRedirect` rendered `<Navigate to={href} replace />` where `href` was **computed each
render** from `useLocation()` (`/t/<ws>/<surface>?<search>`). A render-time `<Navigate>` issues its
navigation as a side effect of rendering; with a freshly-built string `to` every commit, TanStack's
`Transitioner` re-`startTransition`s on each pass before history settles, so the component re-renders
→ re-navigates → React trips its nested-update cap. A **static, typed** `<Navigate to="/t/$ws/channels" params={…}>` (as in `DefaultRedirect`) is stable across renders, so it doesn't loop.

A second, separate slip surfaced first: the initial attempt built the href as `` `#${tenantPath(...)}` `` — **double-prefixing the hash**. Under `createHashHistory()` the `to`/`href` must be the *bare* path (`/t/<ws>/…`); hash history adds the `#`. The `#`-prefixed value didn't match any route and contributed to the thrash.

## Fix

Fire the redirect **once**, imperatively, from an effect — and return `null`:

```tsx
function TenantlessRedirect() {
  const ctx = useAppRoutingContext();
  const location = useLocation();
  const navigate = useNavigate();
  const surface = surfaceForPath(location.pathname);
  const href = `${tenantPath(ctx.workspace, pathForSurface(surface))}${location.searchStr}`;
  useEffect(() => {
    void navigate({ to: href, replace: true });
  }, [href, navigate]);
  return null;
}
```

`href` is the bare path (no `#`); the `/t/$ws` layout guard (`beforeLoad`) likewise uses
`redirect({ href: `${tenantPath(...)}${location.searchStr}` })`. After the change: **0** loop
warnings, 83 gateway + 27 unit tests green.

## Regression guard

`ui/src/App.gateway.test.tsx` → "rewrites a pasted link whose `/t/<ws>` targets another workspace to
the recipient's own" exercises the tenant-less/mismatch redirect path through the real shell; the
back/forward/reload test exercises repeated tenant-prefixed navigation. A reintroduced render-loop
re-floods stderr (and risks an actual hang), caught by these.

## Lesson

In TanStack Router, a redirect whose target is *computed* belongs in a route `beforeLoad`
(`throw redirect(...)`) or an imperative `useEffect` + `navigate(...)` — **not** a render-time
`<Navigate>` with a dynamic `to`. Reserve render-time `<Navigate>` for static, typed destinations.
