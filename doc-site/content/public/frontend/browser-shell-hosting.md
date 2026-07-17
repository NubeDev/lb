---
title: Hosting a browser shell — the SPA fallback and the /api/* session seam
description: How an embedder lets lb's gateway serve its SPA and terminate a cookie-backed browser session, so a deployed shell can actually log in.
---

# Hosting a browser shell

`BootConfig::static_root` lets an embedding host hand lb a built browser shell and have the gateway
serve it — on a single-binary ARM/Pi deploy, lb's gateway **is** the only web server in the process.
Two things are needed for a shell served that way to actually log in, and lb now does both.

Neither is on unless you ask for it. A node with no `static_root` and no `browser_session` keeps
today's bearer-only routing byte-for-byte.

## 1. SPA routes that collide with an lb route (the `/login` 405)

`static_root` mounts the shell on the router's **fallback**, which axum reaches only when *no route
matched the path at all*. Any SPA route sharing a path with an lb route of a **different method**
therefore never reached the shell — the router 405'd first. `GET /login` is exactly that case, since
lb registers `POST /login`: the shell served every other deep link and could not render a login page.

The gateway now content-negotiates on the method-mismatch path, but only when `static_root` is set:

> A request that method-mismatches every handler for its path is served `index.html` **iff** it is a
> `GET`/`HEAD` **and** its `Accept` header *explicitly* prefers `text/html`. Otherwise it gets the
> `405` it always got, `Allow` header intact.

That keys on who is asking, not on which path. A browser navigation always sends
`Accept: text/html,…`; `curl`, the Rust client, and every API consumer do not — so the browser gets
its SPA and the API keeps its contract, with no path list and no host knowledge.

**What this means for you:**

- Your SPA may own `/login` (or any path lb registers under another method). It just works.
- `curl -X GET /mcp/call` still returns `405 Allow: POST`. `Accept: */*` is deliberately **not**
  treated as an HTML preference.
- A *browser* hand-navigating to a POST-only API route will get the shell instead of a 405. That is
  the deliberate cost of content negotiation, and the right trade — API clients never send that
  header.

## 2. A browser session that never holds the token (`/api/*`)

lb's gateway is bearer-only, which is right for a CLI or a sibling node and wrong for a browser: the
token is the whole authority, so anywhere JS can read it, one XSS is a total account compromise. lb
also mints a **fat** token — the full resolved cap set, ~4–9KB — which is over the browser cookie
limit, so "just cookie the JWT" does not merely risk something, it silently fails.

Set `BootConfig::browser_session` and the gateway terminates the session itself:

| Route | Behaviour |
|---|---|
| `POST /api/auth/login` | Runs the real `/auth/login` in-process. Stores the JWT server-side under a fresh opaque session id; sets `lb_session` (`HttpOnly; SameSite=Lax; Path=/`). Returns `principal`, `workspace`, `caps` — **never the token**. |
| `POST /api/auth/select` | Completes the multi-workspace pick (see below). Rotates the sid. |
| `POST /api/auth/switch` | Re-mints into another workspace. Rotates the sid. |
| `POST /api/auth/logout` | Deletes the session row; expires the cookie. |
| `GET /api/auth/session` | The current session's facts, or `401`. |
| `ANY /api/{*rest}` | Resolves the sid to its JWT, attaches `Authorization: Bearer`, and dispatches **internally** to the gateway's own `/{rest}` route. |

The forward is an in-process dispatch into the same router a CLI caller hits — not a loopback hop. One
process, one router, no second port, no second TLS config, and no way for `/api/*` to reach a route
the bearer could not.

**The seam grants nothing.** It attaches a bearer the caller already earned; every downstream route
re-checks capabilities and the workspace wall exactly as it does for a bearer caller. A session in
workspace A cannot reach workspace B's rows through `/api/*`, and a member's session cannot reach an
admin verb — enforced by the same code, not a parallel copy.

### Multi-workspace login

lb's `/auth/login` answers one of three ways, and the seam carries all three through:

- **0 workspaces** → `403`.
- **1 workspace** → the session is established immediately; you get the facts + a cookie.
- **N > 1** → no cookie yet. You get `{select_token, workspaces}` — a short-lived, workspace-less,
  cap-less pre-auth credential — and hand it back to `POST /api/auth/select` with the chosen
  workspace. That call establishes the session.

### CSRF

The moment a cookie authenticates a request, permissive CORS becomes a cross-origin write primitive.
So `/api/*` is **excluded** from the gateway's permissive CORS layer and gated on every unsafe method:
`Sec-Fetch-Site` if the browser sent it (unforgeable by page JS), else `Origin` must match `Host`,
else the request is rejected. "No evidence of same-origin" is not a pass. `SameSite=Lax` is the first
line; this is the second, because `Lax` is a same-*site* check and this is a same-*origin* one.

**This means your shell must be same-origin with the gateway** — which it is, by construction, when lb
serves it via `static_root`. A cross-origin browser app should keep using the bearer contract directly.

### Sessions survive a restart

Sessions are rows in the node's own store with a TTL (12h by default), not a process-local map — so a
deploy does not log everyone out. Session ids are 256-bit CSPRNG values.

## Configuration

```rust
let mut cfg = BootConfig::default();
cfg.static_root = Some("/opt/app/shell".into());       // serve the SPA
cfg.browser_session = Some(BrowserSessionConfig {
    ttl_secs: 60 * 60 * 12,
    secure_cookie: false,   // set true when you terminate TLS
    ..Default::default()
});
```

The standalone `node` binary reads `LB_STATIC_ROOT`, `LB_BROWSER_SESSION`, and
`LB_BROWSER_SESSION_SECURE`.

`secure_cookie` defaults to **false** on purpose: the deploys this exists for are plain-http LAN/Pi
boxes, where a `Secure` cookie is silently dropped by the browser — the same class of invisible
breakage this whole feature fixes. Turn it on when you serve over TLS.

## Not this

- **The bearer contract is unchanged.** `POST /login`, `/auth/*`, and `Authorization: Bearer` are
  untouched. This wraps them; it does not replace or deprecate them.
- **Cookies are not forced on anyone.** A host that holds its token in `localStorage` and talks to the
  gateway cross-origin stays valid — this seam is same-origin, opt-in, and additive. rubixd's own
  embedded UI stays bearer-only.
- **`/api/{*rest}` is not a reverse proxy.** It is a mediated forward to *this* router; it can never
  name an upstream host.
- **Host domain logic stays yours.** lb returns facts (`principal`, `workspace`, `caps`); your shell
  folds them into whatever roles/personas it shows. The cap set is a UI convenience — the boundary is
  always re-checked server-side.
