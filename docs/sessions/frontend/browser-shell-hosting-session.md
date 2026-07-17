# Session — hosting a browser shell: the SPA fallback (#75) + the `/api/*` session seam (#76)

**Date:** 2026-07-17 · **Status:** in-progress ·
**Scopes:** `docs/scope/frontend/spa-static-hosting-scope.md` (#75),
`docs/scope/frontend/browser-session-scope.md` (#76) ·
**Branch:** `scope/browser-shell-hosting`

## What was asked

Fix the two bugs that together make a deployed lb-hosted shell unable to log in — found on ems's
armv7 CM4 target, invisible on a dev box because Vite fills in the missing halves (`NubeIO/ems#8`):

1. **#75** — `GET /login` → `405`. lb registers `POST /login`; axum's `fallback_service` fires only
   when NO route matched, so a method mismatch 405s before the static fallback is ever consulted.
   The login *page* can never render.
2. **#76** — `POST /api/auth/login` → `405 allow:GET,HEAD`. Nothing terminates `/api/*` in
   production; the seam exists only as dev-only Vite middleware in ems and cc-app. The
   *credential* can never post.

## What was found first (and what it changed)

Two facts checked against the real tree before writing code, because both scopes turned on them:

- **#75's "genuinely fiddly part" does not exist.** The scope says `Allow` must be reconstructed
  because "axum does not hand it to the fallback", and calls that *the reason this is a scope and
  not a one-liner*. It is the opposite. `set_allow_header` runs in `RouteFuture::poll` on the way
  **out** (`axum-0.8.9/src/routing/route.rs:164`), wrapping whatever the fallback returned, and
  skips only if the response already carries `Allow`. Verified against real axum before building:

  ```
  GET /login  -> status=418 (custom fallback fired)  allow=Some("POST")   <- attached for free
  ```

  So the handler returns a **bare 405** and lets axum attach the header; hand-setting `Allow` would
  *suppress* axum's correct value. The scope's Risks section is inverted — recorded there.
- **`method_not_allowed_fallback` composes with `fallback_service`** (unmatched paths still reach
  the static tree; `POST /login` untouched), and axum strips the body of a top-level HEAD itself —
  so `HEAD /login` → 200-no-body comes free.

Two corrections to #76's stated placement:

- **`rust/role/gateway/src/session/` already exists** (committed `d2b22a9e`) — it is the
  auth/credential/token-mint module. The scope says to create it new. Dropping cookie/CSRF/forward
  files there would collide two charters — the same mistake the scope explicitly avoids with
  `bootstrap-ui`. Built as a sibling: **`rust/role/gateway/src/browser_session/`**.
- **`BootConfig` does exist** — at `node/src/config.rs:104`, in the `lb-node` crate (not `crates/`).
  An earlier read that said otherwise was wrong; the scope's wording was right, and the field
  landed exactly where it said.

## What was built

### #75 — `spa_fallback.rs` (one file, one responsibility)

`rust/role/gateway/src/spa_fallback.rs` + one mount site in `server.rs`, inside the existing
`match gw.static_root` arm. The rule, in HTTP terms only — no path list, no host knowledge (rule 10):

> method-mismatch **+** `GET`/`HEAD` **+** `Accept` *explicitly* prefers `text/html` → `index.html`.
> Everything else → the 405, `Allow` intact.

`prefers_html` is a named predicate with its own tests precisely because `Accept: */*` (curl's
default) must **not** count — otherwise `curl -X GET /mcp/call` starts returning a web page. `q=0`
is treated as a rejection. `ServeFile` serves the page, so mime/ETag/range match the existing static
fallback exactly (one file-serving path, not a hand-rolled second reader).

### #76 — `browser_session/` (seven files, one responsibility each)

| File | Responsibility |
|---|---|
| `config.rs` | The opt-in switch: TTL + `secure_cookie`. |
| `sid.rs` | 256-bit CSPRNG session ids (`OsRng`) — explicitly not the dev plugins' `s${counter}_${Date.now()}`. |
| `cookie.rs` | Parse/emit: `HttpOnly; SameSite=Lax; Path=/` (+`Secure` by config). |
| `csrf.rs` | `Sec-Fetch-Site` → `Origin`-vs-`Host` → **reject**. The gate. |
| `store.rs` | Store-backed sessions + TTL, in the reserved `_lb_browser_session` namespace. |
| `auth.rs` | `/api/auth/{login,select,switch,logout,session}`. |
| `forward.rs` | `ANY /api/{*rest}` → internal dispatch with the bearer attached. |

Decisions worth recording:

- **Namespace.** A session is looked up by `sid` alone — the cookie is all the browser sends and the
  workspace is what we're trying to *learn*, so it cannot live in a ws-scoped table. It goes in the
  reserved system namespace `_lb_browser_session`, following the existing convention for genuinely
  global records (`lb_authz`'s `IDENTITY_NS = "_lb_identity"`, `_lb_workspaces`,
  `_lb_workflow_directory`). The row is a *lookup*, not an authority: the wall is still enforced
  downstream by the token's own `ws` claim.
- **One login implementation, not a second copy.** `auth.rs` calls the **real** `routes::auth_*`
  handlers in-process and re-shapes their reply, so the uniform 401, timing-uniform argon2, the
  per-email rate limit, and the 0/1/N branch are all inherited. Re-deriving any of it here is the
  third-and-fourth-copy failure the scope exists to prevent. This required exporting `AuthReply` /
  `WorkspaceRow` from `routes/mod.rs`.
- **The 0/1/N branch is preserved, not flattened.** `/auth/login` answers three ways; a seam handling
  only the 1-workspace case would silently break every multi-workspace human. The select-token is
  passed to the browser (a 60s, workspace-less, cap-less pre-auth credential — not the fat JWT) and
  `/api/auth/select` completes the exchange. `/api/auth/switch` attaches the session's stored token
  since the browser holds no bearer.
- **Rotation.** login / select / switch each mint a fresh sid and delete the old row (fixation).
  Rotation happens only *after* the credential check passes, so a failed login can't log someone out.
- **CORS ordering is the whole CSRF posture.** `CorsLayer::permissive()` is applied to the bearer API
  router; the `/api/*` routes are built **separately and merged after**, so they never inherit it.
  Cookie auth under permissive CORS is a cross-origin read primitive. `merge` (not `nest`) keeps the
  static-root fallback answering everything else.
- **No circularity.** `forward` needs the router that `router(gw)` builds. Resolved by passing the
  built router as the `/api/*` routes' own axum state (`ApiState { gw, inner }`) — `Gateway` knows
  nothing about a router.
- **`insert` (not `append`) on `Authorization`** so a browser-supplied bearer can never survive into
  the inner route. The cookie is the only credential the seam honours (pinned by
  `a_smuggled_bearer_is_ignored`).
- **`secure_cookie` defaults false.** The deploys this seam exists for are plain-http LAN/Pi boxes; a
  `Secure` cookie there is silently dropped — the exact "login does nothing" class of bug being fixed.
  `__Host-` is not used for the same reason.

### Config seam

`BootConfig::browser_session: Option<BrowserSessionConfig>` → `Gateway::with_browser_session`,
mirroring `static_root`'s posture exactly. `None` (default) ⇒ the router is byte-for-byte today's.
`from_env` honours `LB_BROWSER_SESSION` / `LB_BROWSER_SESSION_SECURE` for the standalone binary.

## Test evidence

`rust/role/gateway/tests/static_root_method_mismatch_test.rs` (10) — the file that closes the gap
that let #75 ship (`static_root_test.rs` had 5 tests, none for a method mismatch):

```
test browser_navigation_to_login_reaches_the_spa ... ok        <- the ems#8 regression test
test method_mismatch_405_keeps_its_allow_header ... ok         <- trap 1: Allow not dropped
test curl_default_accept_does_not_get_html ... ok              <- trap 2: */* is not html
test without_a_static_root_a_browser_still_gets_the_405 ... ok <- static_root:None unchanged
test api_client_on_a_method_mismatch_still_gets_405 ... ok
test no_accept_header_does_not_get_html ... ok
test browser_navigating_to_an_api_route_gets_the_shell_by_design ... ok
test head_navigation_serves_index_with_no_body ... ok
test a_non_get_method_mismatch_is_never_html ... ok
test the_real_post_route_is_unaffected ... ok
test result: ok. 10 passed; 0 failed
```

`browser_session_csrf_test.rs` (6) — **the gate on #76 shipping**, plus both mandatory categories:

```
test a_cross_origin_post_with_a_valid_cookie_is_rejected ... ok  <- THE gate
test the_same_call_same_origin_succeeds ... ok
test a_foreign_origin_without_sec_fetch_site_is_rejected ... ok
test a_post_with_no_origin_evidence_is_rejected ... ok
test capability_deny_holds_through_the_seam ... ok               <- mandatory
test workspace_isolation_holds_through_the_seam ... ok           <- mandatory
test result: ok. 6 passed; 0 failed
```

`browser_session_test.rs` (10):

```
test token_never_reaches_the_browser ... ok        <- the whole point of the scope
test a_smuggled_bearer_is_ignored ... ok
test a_session_survives_a_gateway_rebuild ... ok   <- restart survival (not a process map)
test the_seam_is_absent_unless_configured ... ok   <- off by default
test login_sets_an_httponly_cookie_and_returns_facts ... ok
test bad_password_sets_no_cookie ... ok
test a_forged_sid_is_rejected ... ok
test an_expired_session_is_rejected ... ok
test logout_kills_the_session ... ok
test forward_dispatches_with_the_sessions_bearer ... ok
test result: ok. 10 passed; 0 failed
```

Plus 18 unit tests in-crate (`prefers_html` × 5, cookie × 5, csrf × 7, sid × 2 — 13 under
`browser_session`, 5 under `spa_fallback`).

All on the real gateway, real SurrealDB (`mem://`), real argon2, real caps, seeded through the real
write path (`/login` bootstrap → `/admin/identities` → `/admin/identities/{sub}/password` →
`/admin/members`). No mocks, no fake backend (CLAUDE §9).

**Known unrelated failure:** `publish_install_test` does not compile — it needs a wasm build artifact
(`extensions/hello-v2/target/wasm32-wasip2/release/hello_v2_ext.wasm`) that is not git-tracked and
requires a separate build step. It references none of this work; pre-existing.

## Capability posture

`/api/*` grants nothing. It attaches a bearer the caller already earned at login; every downstream
route runs its own `authenticate` + cap check unchanged, because the forward dispatches into the
**same router** a CLI caller hits. That is why the seam is structurally incapable of widening — and
why the deny/isolation tests pass without `/api/*`-specific cap code existing at all.

#75 is transport-layer, strictly before auth, and leaks no authenticated byte: `index.html` is the
same public shell already served at `/`.

## Docs reconciled

`docs/scope/deploy/rubixd/token-auth-scope.md` and `embedded-ui-scope.md` say "no cookies, no
sessions". Both annotated: they are the **rubixd** surface (its own localhost-bound fleet-agent UI),
the seam is opt-in and `None` there, so both lines remain true where written. Without the annotation
this reads as a reversal.

## Still open

- Public promotion to `doc-site/content/public/`.
- `STATUS.md`.
- The scopes' open-questions sections (#75's `Allow` risk needs correcting — it is inverted).
- The tag carrying seed_email + `/auth/*` (master is 21 commits ahead of `node-v0.4.6`); ems#8 step 2
  follows it. **Not cut until both land** — the CSRF gate is the scope's own merge condition.
