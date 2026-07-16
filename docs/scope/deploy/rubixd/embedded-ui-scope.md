# rubixd scope — embedded Bootstrap UI

Status: scope (the ask). Slice 7 of [`README.md`](README.md); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md).

A **small, build-step-free web UI** embedded in the rubixd binary: Bootstrap 5 +
vanilla JS, served by the existing axum server via `rust-embed`. It is a *thin lens
over the REST surface* — the UI adds **zero** server verbs; anything it can do, curl
with the same bearer token can do (that is the slice-2 contract).

## Goals

- `crates/rubixd/ui/` static assets embedded at compile time: `index.html`, pages,
  `bootstrap.min.css/js` (vendored, pinned — **no CDN**: edge boxes are offline),
  `app.js` (fetch + render, no framework, no npm, no bundler).
- Pages:
  - **Claim** (`/claim`) — the one-time token claim: a single button (plus the 6-digit
    code field when the bind is non-local), shows the token **once** with a copy
    button and a "store this now" warning, then stores it in `localStorage` and moves
    to the dashboard. Re-visiting after claim shows "already claimed — paste your
    token" (login box).
  - **Dashboard** (`/`) — machine summary: bundles, instances table (package, version,
    backend, health badge green/amber/red, last transition), poller status (last
    check, next check, rartifacts reachability).
  - **Instance detail** (`/instances/<name>`) — kept versions, bad-version marks with
    a *Clear* button, env keys (values redacted), and the **Rollback** button
    (confirm dialog → `POST /api/instances/{name}/rollback`).
  - **Apply** (`/apply`) — paste/upload a bundle YAML → `POST /api/bundles/apply`,
    rendering validation errors verbatim.
- Auth handling in `app.js`: every fetch sends `Authorization: Bearer` from
  `localStorage`; 401 → redirect to login box; no cookies, no sessions.
- Read-only auto-refresh (poll `GET /api/status` every 5 s while visible) — no SSE/WS
  machinery for v1.

## Non-goals

- No React/Vite/Tailwind/shadcn here — that is rartifacts' UI. rubixd's UI must add
  ~zero build complexity to an edge agent (the "small embedded UI" ask, literally).
- No fleet view (one machine only), no log viewer (journalctl exists), no user
  management (one token), no charts.

## Intent / approach

Static files + `fetch` keeps the agent binary boring: no node toolchain in the rubixd
build, no asset pipeline to break cross-compiles (armv7!). Vendored Bootstrap satisfies
the zero-third-party-JS line the token scope demands (XSS surface = our own `app.js`
only; render everything with `textContent`, never `innerHTML` with server data).
Alternative rejected: server-side templates (askama) — the REST surface must stay the
single source of truth, and JSON+JS keeps it honest.

## How it fits the core

Rule "the menu is not the permission model" translated: the UI holds no authority — the
token does; every destructive action is a REST call that would 401 without it. One
responsibility per file applies to `ui/` too (one JS module per page).

## Example flow

1. Fresh box: journal prints the claim URL → operator opens `/claim` → token shown
   once, copied → dashboard renders two instances green.
2. A bad 0.4.6 rolls back overnight → dashboard shows `rubix-lab` amber with
   `0.4.6 bad`; operator opens detail, reads the transition error, clicks *Clear* after
   the fix ships.
3. Operator pastes an updated bundle in `/apply` → validation error (colliding port)
   rendered → fixes → applies → watches the instance go green.

## Testing plan

- The REST contract the UI rides is already covered by slices 2/4/6 — re-assert here
  that the UI introduced no new routes (route-table snapshot test).
- Embedded-asset test: binary serves `/claim`, `/`, assets 200 with correct
  content-types, no external URLs in any served asset (grep the embedded bundle for
  `http://`/`https://` — offline guarantee).
- Browser smoke (real rubixd, headless browser in CI): claim happy path (token visible
  once, second visit shows login), 401 redirect on bad token, rollback button fires
  the POST and re-renders.

## Risks & hard problems

- Token in `localStorage` (accepted in the token scope) — the mitigations live here:
  vendored-only JS, `textContent` rendering, CSP header (`default-src 'self'`) on
  every UI response.
- Headless-browser CI flakiness — keep the smoke to 3 paths; everything else is REST
  tests.

## Open questions

- Dark mode: Bootstrap 5.3 `data-bs-theme` is nearly free — include? Recommendation:
  yes, auto via `prefers-color-scheme`, no toggle.

## Related

[`token-auth-scope.md`](token-auth-scope.md) (claim + bearer contract) ·
[`../rartifacts/web-ui-scope.md`](../rartifacts/web-ui-scope.md) (the rich-UI sibling
and the deliberate contrast).
