# Frontend scope — `static_root` must host a real SPA, not just unmatched paths

Status: scope (the ask). Owning repo: **lb** (this repo).

`BootConfig::static_root` exists so an embedding host can hand lb a built browser shell and let the
gateway serve it — ems's ARM/Pi deploy does exactly this, and it is the only web server in that
process. The fallback is wired as `ServeDir::new(dir).fallback(ServeFile::new(index.html))` on
`Router::fallback_service`, which axum reaches **only when no route matched the path at all**. Any
SPA route that shares a path with an lb API route of a *different method* therefore never reaches the
shell: the router 405s first. `GET /login` — the single most conventional path an SPA can own — is
exactly this case, because lb registers `POST /login`. The result is a deployed product that serves
its shell, answers every other deep link, and cannot show a login page. We want `static_root` to mean
"lb hosts this SPA correctly," with API semantics for API clients left exactly as they are.

## Goals

- **A browser navigation to any path an SPA owns reaches `index.html`**, including paths that collide
  with an lb route registered under a different method (`/login`, and any future one).
- **API semantics are unchanged for API clients.** `GET /mcp/call` must still be `405 Allow: POST` —
  it must NOT start returning `200 text/html`. A 405 that turns into an HTML page is a debugging
  nightmare and an API contract break.
- **Generic.** No host knowledge, no path list, no `if ems`. Fixing `/login` by naming `/login` is
  not a fix.
- **Off unless `static_root` is set.** A node with no shell keeps today's routing byte-for-byte.

## Non-goals

- Renaming or retiring lb's legacy `POST /login`. It is load-bearing (`clients/rust/src/client.rs:55`,
  `identity_routes_test.rs`, `admin_routes_test.rs`, `nav_reach_test.rs`, and ems's own Makefile
  targets) and a breaking rename is a worse trade than fixing the fallback.
- The `/api/*` session seam. That is a separate, larger ask — `browser-session-scope.md`. This scope
  is only about which handler answers a **navigation**.
- Changing `ServeDir`/caching/precompression behaviour.

## Intent / approach

**Content negotiation on the method-mismatch path**, at the one router-construction site
(`rust/role/gateway/src/server.rs:501-511`), only when `static_root` is `Some`:

> When a request method-mismatches every registered handler for its path, serve `index.html` **iff**
> it is a `GET`/`HEAD` **and** its `Accept` header prefers `text/html`. Otherwise return the 405 the
> router would have returned, `Allow` header intact.

That rule is precise about who is asking. A browser navigation always sends `Accept:
text/html,application/xhtml+xml,…`; `curl`, the Rust client, and every API consumer do not. So the
browser gets its SPA and the API keeps its contract — no path list, no host knowledge.

axum 0.8.9 (pinned) exposes exactly one hook for this: `Router::method_not_allowed_fallback`
(`axum-0.8.9/src/routing/mod.rs:374`). **Note the signature — it takes `H: Handler`, not a
`Service`**, so `ServeDir` cannot be passed to it directly the way `fallback_service` takes it. The
handler is a small async fn that reads the method + `Accept`, and either replies with the
`ServeFile`'s bytes or reconstructs the 405. The `Allow` header must be preserved: axum does not hand
it to the fallback, so it has to be derived from the router — **this is the one genuinely fiddly part
and the reason this is a scope and not a one-liner** (see Risks).

**The alternative rejected:** having ems rename its SPA route to `/signin`. It is one line and it
works, and it is a rule-10 violation dressed as a fix: it leaves lb's `static_root` contract broken
for cc-app and every future embedder, silently, until each one rediscovers it in a browser on a
target box. The `/assets/{id}` precedent looks like it argues the other way but does not — that was
GET-vs-GET, where lb genuinely cannot know which resource the browser wanted, so renaming the
host-owned artifact (`ems/ui/vite.config.ts` → `app-assets/`) was the only correct answer. Here lb has
**no GET handler for `/login` at all**; there is nothing to disambiguate, and lb can be right for
everyone.

## How it fits

- **Rule 10 / no special-casing:** the fix reaches no host and names no path. `static_root` stays a
  generic seam; the rule is expressed in HTTP terms (method + `Accept`), not product terms.
- **Placement:** one site, `rust/role/gateway/src/server.rs`, inside the existing
  `match gw.static_root` arm. The handler itself is its own file under `rust/role/gateway/src/`
  (one responsibility per file), not inlined into `server.rs`.
- **Capabilities:** none. This is transport-layer routing, strictly before auth. It must not leak an
  authenticated byte — `index.html` is the same public shell already served at `/`.
- **Symmetric nodes:** behaviour is driven by `BootConfig::static_root`, not a role branch.
- **No mocks:** tests boot the real gateway with a real temp `static_root` dir.

## Example flow

1. ems's node boots with `static_root = .ems/embedded-ui/shell`.
2. Browser navigates to `http://pi:8391/login`. axum matches the path `/login`, finds only a `POST`
   handler, and hands the request to the method-not-allowed fallback.
3. The fallback sees `GET` + `Accept: text/html…` → responds `200` with `index.html`.
4. The SPA boots, its router reads `/login`, renders `LoginPage`. **The user can sign in.**
5. Meanwhile `curl -X GET http://pi:8391/mcp/call` sends no `Accept: text/html` → still
   `405 Allow: POST`. The Rust client is unaffected.

## Testing plan

Extends the existing `rust/role/gateway/tests/static_root_test.rs` (5 tests today, none of which
cover a method mismatch — that gap is precisely why this shipped broken):

- `GET /login` with `Accept: text/html` + `static_root` set → `200`, body is `index.html`. **The
  regression test for this bug.**
- `GET /login` with `Accept: application/json` → `405`, `Allow: POST` intact.
- `GET /mcp/call` with `Accept: text/html` → **`405`**, not HTML. (The API-semantics guard: proves
  the rule keys on method-mismatch, not on "browser asked".) *See Open→Risks note.*
- `POST /login` (the real route) → unchanged behaviour, still mints a token.
- `GET /sites` (matches nothing) → `200` index.html, i.e. the existing `fallback_service` path is
  untouched.
- **`static_root = None`** → `GET /login` is `405`; no static behaviour anywhere. Byte-for-byte
  today's routing.
- `HEAD /login` → `200`, no body.

## Risks & hard problems

- ~~**Reconstructing `Allow`.**~~ **RESOLVED — this risk was inverted, and it is the reason this was
  scoped rather than one-lined.** axum's `method_not_allowed_fallback` does not pass the allowed-method
  set *into* the handler, but it does not need to: `set_allow_header` runs in `RouteFuture::poll` on the
  way **out** (`axum-0.8.9/src/routing/route.rs:164`), wrapping whatever the fallback returned, and it
  skips only when the response *already* carries `Allow`. Verified against real axum 0.8.9 before
  implementing (`GET /login` → custom fallback fired, `allow=Some("POST")` attached automatically). So
  the handler returns a **bare 405** and gets the correct `Allow` for free; **hand-setting `Allow` in the
  handler would SUPPRESS axum's real value** — that is the actual trap. The `Allow: POST` assertion is
  still not decorative (`method_mismatch_405_keeps_its_allow_header` pins it), because a future refactor
  that starts setting the header itself would silently regress it.
- **The `GET /mcp/call` case is the real tension.** A browser hitting a POST-only API route by hand
  will now get HTML instead of 405. That is the deliberate cost of content negotiation, and it is the
  right trade: API clients never send `Accept: text/html`, and an SPA's navigations always do. Worth
  stating in the scope so the next reader doesn't "fix" it.
- **`Accept: */*`** (curl's default) must NOT be treated as html-preferring, or `curl -X GET /mcp/call`
  starts returning a web page. The check is "explicitly prefers `text/html`", not "doesn't object to
  it". This is the most likely implementation bug.

## Open questions

None. **Shipped** — see `docs/sessions/frontend/browser-shell-hosting-session.md`.

The rule (method-mismatch + GET/HEAD + explicitly prefers `text/html` → `index.html`, else the 405
unchanged) was decided here and built as stated. The one open mechanism — `Allow` reconstruction —
turned out not to need reconstructing at all (see Risks, resolved): axum re-attaches it to the
fallback's response, so the handler returns a bare 405. Built in `rust/role/gateway/src/spa_fallback.rs`;
covered by `rust/role/gateway/tests/static_root_method_mismatch_test.rs` (10 tests, including the
`static_root: None` byte-for-byte guard and both traps).

## Related

- `browser-session-scope.md` — the `/api/*` session seam. **Both are required for a deployed shell to
  log in**; this one alone gets the page to render, not the credential to post.
- `rust/role/gateway/src/server.rs:501-511` — the site.
- `rust/role/gateway/tests/static_root_test.rs` — the suite that missed it.
- `NubeIO/ems` `docs/scope/auth-login/deployed-login-scope.md` — the downstream consumer, and the
  live evidence (found on an ARM/Pi target, invisible on the dev box because Vite proxies).
- `NubeIO/ems` `docs/scope/ui/ui-architecture.md:120-124` — the `/assets/{id}` precedent, and why it
  does **not** apply here.
