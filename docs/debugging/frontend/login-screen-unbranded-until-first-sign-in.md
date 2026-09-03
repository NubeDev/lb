# Login screen is unbranded on a never-signed-in browser

- Area: frontend / gateway (pre-auth)
- Status: resolved (lb half; the shell half is a rubix-ai follow-up)
- First seen: 2026-09-03 ([NubeIO/rubix-ai#306](https://github.com/NubeIO/rubix-ai/issues/306))
- Resolved: 2026-09-03
- Session: ../../sessions/frontend/public-branding-route-session.md
- Regression test: `rust/role/gateway/tests/public_branding_route_test.rs`

## Symptom

On a browser that has **never** signed in to a deployment, the sign-in screen shows none of the
workspace's branding: the neutral centered card, no customer logo, the shipped Nube iO tab title and
favicon, and the product default theme instead of the workspace's. Sign in once and the branding
appears from then on — including on the sign-in screen after a sign-out. So the deployment looks
correctly branded to everyone who has already used it, and wrong to every first-time visitor, which
is exactly the audience the brand is for.

## Reproduce

On the ESR deployment:

1. Clear site data for the host (or open a fresh profile / a new device).
2. Load the site → the neutral card, product favicon, product default theme.
3. Sign in → "ESR / building intelligence", the ESR mark, the ESR favicon, and the `split` login
   layout the brand selects.
4. Every later visit **in that browser** is branded, sign-out included.

Step 4 is the tell: the branding is per-browser, not per-deployment.

## Investigation

Not a rendering bug, and nothing was broken — the shell was doing the only thing it could.

- Every `prefs.*` verb derives its workspace from the bearer token, so **pre-auth there is no verb to
  call**: the sign-in screen has no token, therefore no workspace, therefore no brand.
- The brand the shell paints pre-auth comes entirely from `localStorage` (`lb.brand.<ws>`), written by
  the first *authenticated* `prefs.resolve`. `resolveBootBrand` picks the best candidate out of that
  cache — the URL `#/t/<ws>` hint, then the typed email's last workspace, then the sole cached brand.
- With an empty cache there is no candidate and it falls back to `DEFAULT_BRANDING`. Working as
  written (see `sessions/frontend/login-branding-session.md`, which shipped exactly this and flagged
  the gap as the open follow-up) — but the cache can only be seeded by a prior signed-in visit, and a
  new device, a new browser or a cleared profile has no prior visit by definition.

Ruled out: a theme-provider bug. The *other* half of the original report — the workspace theme not
applying after the first sign-in until a page reload — was a genuinely separate mount-only-effect bug
in `ThemeProvider`, fixed downstream on `feat/site-map-slot-links`. It is not this.

## Root cause

**Missing capability, not a defect.** There was no unauthenticated way to read a workspace's brand,
so the first paint on a cold browser could never be right. `workspace-branding-scope.md` had named
this seam from the start and deliberately deferred it, shipping the `localStorage` cache instead.

## Fix

The deferred seam, built: `GET /public/branding?ws=<ws>` in the gateway
(`rust/role/gateway/src/routes/public_branding.rs`), unauthenticated, returning the workspace-default
`ui_branding` + `ui_theme` blobs and nothing else. Fixed at the gateway because that is the only layer
that can serve a no-principal read; no shell change can substitute for it.

It is a deliberate read-only break in the workspace wall, so the fix is bounded by four tested
invariants — whitelist by **construction, not filtering**; every miss answers **byte-identically** (no
workspace-existence oracle); **`ws` required, never inferred** (inferring means enumerating workspaces
for an anonymous caller); rate-limited per client on its own budget. Rationale in the module docs.

## Verification

`cargo test -p lb-role-gateway --test public_branding_route_test` — **5 passed**, real node, real
store, no `Authorization` header anywhere in the file.

## Prevention

The regression tests are written as the wall's guard rather than a happy path, so the *class* of
mistake is what fails:

- `body_carries_the_brand_axes_and_nothing_else` sets **every other prefs axis** on the record and
  asserts each is absent from the raw body — a future axis added to `Prefs` cannot leak by existing.
- `every_miss_answers_identically` compares the **bodies** of five different misses, not their status
  codes, so re-introducing an oracle fails.
- `serves_only_the_named_workspaces_brand` is the mandatory workspace-isolation test.

Guardrail in code: the handler destructures `Prefs` into two named fields and never serializes the
record, so widening the response requires editing that file by hand — which is the review this route
exists to force.
