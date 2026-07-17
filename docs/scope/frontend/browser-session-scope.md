# Frontend scope — a browser session seam (`/api/*`) for hosts that serve a shell

Status: scope (the ask). Owning repo: **lb** (this repo).

lb's gateway is bearer-only: `POST /login` mints a JWT and every route wants
`Authorization: Bearer`. That is the right contract for a CLI, a sibling node, or rubixd. It is not a
contract a **browser** can hold safely: the token is the whole authority, and putting it anywhere JS
can read it makes every XSS a total account compromise. lb also mints a *fat* token — the full
resolved cap set, ~4–9KB — which is over the browser cookie limit, so "just cookie the JWT" is not
merely unwise, it silently fails. Two hosts (ems, cc-app) independently hit this, and both solved it
the same way: a **dev-only Vite middleware** that keeps the token server-side, cookies a short opaque
session id, and forwards `/api/*` to the gateway with the bearer attached. Neither has a production
equivalent, because neither has a process to put one in — **lb's gateway is already their web server**
(`static_root`). The result: ems's ARM/Pi build serves its shell and cannot log in, and cc-app will
hit the identical wall the day it gets a deploy milestone. We want lb to own this seam, opt-in, so a
host that serves a shell gets a working browser session without hand-rolling a security boundary.

## Goals

- **A browser never holds a bearer token.** The session id in an `HttpOnly` cookie is the only thing
  in the browser; the JWT stays server-side. XSS can then ride the session, but cannot *exfiltrate*
  the credential — a materially better position than `localStorage`.
- **`/api/*` is a real seam in production**, terminated by lb when a host opts in, with the same shape
  the two dev plugins already prove: login/logout, and a mediated forward of everything else.
- **One implementation, not one per host.** `ems/ui/vite-dev-auth.ts` and `cc-app/ui/vite-dev-auth.ts`
  are already near-twins of a security-critical seam. A third and fourth copy in Rust is the failure
  mode this scope exists to prevent.
- **Opt-in and inert by default.** `static_root`-less nodes, rubixd, and rubix-ai keep today's
  bearer-only behaviour byte-for-byte.
- **Sessions survive a restart.** Store-backed with a TTL, not a process-local `HashMap` (which is
  correct for a dev plugin and wrong for a product: every deploy logs everyone out).

## Non-goals

- **Changing the bearer contract.** `POST /login`, `/auth/*`, and `Authorization: Bearer` are
  untouched. This scope *wraps* them; it does not replace or deprecate them.
- **Forcing cookies on anyone.** rubix-ai deliberately holds its token in `localStorage`
  (`ui/src/lib/session/session.storage.ts:10`) and talks to the gateway cross-origin. That stays
  valid; this seam is same-origin, opt-in, and additive.
- **Host domain logic.** `roleFromCaps`, the default workspace, and which personas exist are the
  host's. lb returns the facts (`principal`, `workspace`, `caps`); the shell folds them.
- **The `/login` SPA collision.** Separate ask — `spa-static-hosting-scope.md`.
- **A new identity model.** No new principals, no new grants, no change to membership resolution.

## Intent / approach

**A `browser_session` layer on the gateway router, enabled from `BootConfig`, sitting in front of
`/api/*` only.** When `BootConfig::browser_session` is `None` (the default) the router is exactly
today's. When `Some(cfg)`, the gateway mounts:

| Route | Behaviour |
|---|---|
| `POST /api/auth/login` | Invoke the existing `/auth/login` (or `/login`) handler **in-process** — not over the loopback. Store the minted JWT in the session store under a fresh opaque sid. Set `HttpOnly; SameSite=Lax; Path=/` (+`Secure` when TLS). Return the public session facts (`principal`, `workspace`, `caps`, `locale`) — never the token. |
| `POST /api/auth/logout` | Delete the session row; expire the cookie. |
| `GET /api/auth/session` | The current session's facts, or `401`. Replaces each host's hand-rolled `/api/me/workspaces`. |
| `ANY /api/{*rest}` | Resolve sid → JWT; attach `Authorization: Bearer`; dispatch **internally** to the gateway's own `/{rest}` route. `401` when the sid is absent/expired. |

The forward is an internal dispatch, not an HTTP hop to itself — one process, one router, no loopback
port, no second TLS config, and no way for `/api/*` to reach a route the bearer wouldn't have.

**Session store:** an lb store table (`session` rows: sid, token, principal, ws, `expires_at`), so
sessions survive restart and can be revoked. The sid is a CSPRNG value — explicitly *not* the dev
plugins' `s${counter}_${Date.now()}`, which is guessable and fine only because it never leaves a dev
box.

**CSRF is the load-bearing risk, not an afterthought.** The moment a cookie authenticates a request,
`CorsLayer::permissive()` (`server.rs:515`) becomes a cross-origin write primitive. So: `/api/*` is
**excluded from the permissive CORS layer** and gets its own strict layer — same-origin only, plus an
`Origin`/`Sec-Fetch-Site` check on every unsafe method, on top of `SameSite=Lax`. A scope that adds
cookie auth without this is a CSRF hole with good intentions.

**The alternative rejected:** each host builds its own Rust BFF. It sounds like it respects the
"hosts own their shell" line, but a host has no process to build it in — lb's gateway *is* the server
via `static_root`, so an ems-side BFF means standing up a second HTTP server in front of lb purely to
add a cookie. Two servers, two TLS configs, two CORS policies, one extra hop, and three divergent
copies of a session boundary. The `>4KB fat JWT` that forces the whole design is lb's own doing; the
mitigation belongs next to the cause.

**A second alternative rejected:** follow rubix-ai and put the bearer in `localStorage`, deleting the
`/api` seam entirely. It is the least code and has real precedent in the family. It is rejected on
posture: ems and cc-app both chose the cookie shape deliberately, and moving *toward* a JS-readable
fat token — one carrying the full cap set — is a security regression we would be adopting for
convenience. rubix-ai's own defence ("the gateway re-checks every verb server-side") answers
authorization, not token theft.

## How it fits

- **Rule 10 / no special-casing:** lb learns nothing about ems or cc-app. The layer knows only
  "there is a shell, sessions are cookies" — expressed as `BootConfig`, per **rule 2 (role = config,
  never a code branch)**.
- **Capabilities & the deny path:** `/api/*` grants nothing. It attaches a bearer the caller already
  earned; every downstream route re-checks caps exactly as today. **The deny test that matters:** a
  session for workspace A calling an `/api/*` verb scoped to workspace B is denied by the same
  membership/cap check as the bearer path — the seam must be provably incapable of widening.
- **Isolation/tenancy:** the sid resolves to one principal + one workspace. Workspace switching goes
  through the existing `/auth/switch`, re-minting into the same session row.
- **Placement:** a new module under `rust/role/gateway/src/session/` (one responsibility per file:
  cookie parse/emit, store, the forward, the CORS/CSRF layer). **Not** `bootstrap-ui` — that crate is
  a 5-line placeholder for the first-run super-admin UI (README §6.13), a different charter; squatting
  in it would confuse two asks.
- **Data:** one new store table + TTL sweep. **Secrets:** the JWT becomes data at rest in the store —
  it already is in the dev map; the store is the more defensible home.
- **Prior art to reconcile:** `docs/scope/deploy/rubixd/token-auth-scope.md:54` and
  `embedded-ui-scope.md:30-31` say "no sessions/cookies — the bearer is the whole story." Those are
  **rubixd** scopes (a fleet agent's own embedded UI), not the app-shell surface, and this seam is
  opt-in, so they remain true where they were written. **Both must be annotated** to say so, or the
  next reader will read this as a contradiction — which, undocumented, it would be.

## Example flow

1. ems's node boots with `static_root` + `browser_session: Some(..)`.
2. Shell POSTs `/api/auth/login` `{email, password}` — no token, no gateway URL, same origin.
3. lb calls its own `auth_login` in-process → JWT + caps. Writes `session{sid, token, …, expires_at}`.
   Responds `Set-Cookie: lb_session=<csprng>; HttpOnly; SameSite=Lax` + `{principal, workspace, caps}`.
4. Shell folds `caps` → its own coarse admin/member UI signal. **No token in JS. Ever.**
5. Shell POSTs `/api/mcp/call` with the cookie. lb resolves sid → JWT, attaches the bearer, dispatches
   internally to `/mcp/call`, which cap-checks exactly as it does for a CLI caller.
6. Restart the node. The cookie still works — the session is in the store. (Today's dev map: logged out.)
7. `POST /api/auth/logout` → row deleted, cookie expired, the JWT is dead to the browser.

## Testing plan

Real gateway, real store (`mem://`), real routes — no mocks (rule 9).

- **Capability-deny (mandatory):** a session whose caps lack `mcp:x:call` gets the same `403` through
  `/api/*` as through the bearer path. *The seam must not widen anything.*
- **Workspace-isolation (mandatory):** a session in ws A cannot reach ws B's rows via `/api/*`.
- **The token never reaches the browser:** assert no response body/header on any `/api/*` route ever
  contains the JWT. A grep-style guard, because this is the whole point of the scope.
- Login → cookie set, `HttpOnly` present, body carries no token.
- Bad password → `401`, no cookie, no session row.
- Forged/unknown/expired sid → `401`, never a 500, never an anonymous pass-through.
- **Restart survival:** mint a session, rebuild the gateway against the same store, reuse the cookie → still `200`.
- **CSRF:** a cross-origin `POST /api/mcp/call` carrying a valid cookie is **rejected**; the same call
  same-origin succeeds. Plus: `/api/*` must not be covered by `CorsLayer::permissive`.
- **Off-by-default:** `browser_session: None` → `/api/auth/login` is `404`; no `Set-Cookie` exists
  anywhere; every existing gateway test passes untouched.
- TTL expiry → `401`.

## Risks & hard problems

- **CSRF vs `CorsLayer::permissive()`.** The single largest risk. Cookie auth under permissive CORS is
  exploitable by default; the strict `/api/*` layer is not optional, and the cross-origin test above is
  the gate on this scope shipping at all.
- **Session fixation / rotation.** The sid must be re-minted on privilege change (`/auth/switch`,
  `/auth/select`), or a pre-login sid survives into an authenticated session.
- **This is a security boundary in a repo that has never had one of this shape.** lb has no cookie code
  anywhere today (`grep -rn cookie rust/**/*.rs` → zero). Reviewer attention should be
  disproportionate; prefer the boring, well-trodden shape over anything clever.
- **Scope creep toward a general reverse proxy.** `/api/{*rest}` is a mediated forward to *this*
  router, not a proxy to arbitrary upstreams. Keep it that way.
- **Fat-JWT growth.** This mitigates the >4KB token; it does not fix it. If the cap set keeps growing,
  the token is still a problem for the bearer path (headers, logs). Out of scope, worth its own ask.

## Open questions

None blocking. Decided: lb owns it; opt-in via `BootConfig`; store-backed sessions; internal dispatch
rather than a loopback hop; strict CORS on `/api/*`; hosts keep their own cap→role folding.

## Related

- `spa-static-hosting-scope.md` — the `/login` 405. **Both are required**: that one renders the login
  page, this one makes the credential post work. Shipping either alone leaves login broken.
- `docs/scope/deploy/rubixd/token-auth-scope.md:54`, `embedded-ui-scope.md:30-31` — the "no cookies"
  positions this must annotate (rubixd surface, still true there).
- `rust/role/gateway/src/server.rs:85` (`POST /login`), `:91-94` (`/auth/*`), `:501-515` (`static_root`
  + `CorsLayer::permissive`).
- Reference implementations to converge (both dev-only, both TS): `NubeIO/ems`
  `ui/vite-dev-auth.ts`, `NubeIO/cc-app` `ui/vite-dev-auth.ts` — the latter is ahead (invite
  verify/accept) and should be read before fixing the route list.
- `NubeIO/ems` `docs/scope/auth-login/deployed-login-scope.md` — the downstream consumer.
- `NubeIO/cc-app` `docs/sessions/HANDOVER-email-login-e2e.md:122-123` — "don't over-invest the dev seam
  into a prod BFF **unless the scope says so**." This is that scope saying so.
