# Session — the pre-auth public branding route

Scope: [`docs/scope/frontend/workspace-branding-scope.md`](../../scope/frontend/workspace-branding-scope.md)
(the deferred "public read seam" slice).
Predecessor: [`login-branding-session.md`](./login-branding-session.md) — the `localStorage`-cache
slice this closes the gap in.
Downstream report: [NubeIO/rubix-ai#306](https://github.com/NubeIO/rubix-ai/issues/306).

## Ask

On a browser that has **never** signed in to a deployment, the sign-in screen is unbranded: the
neutral card, no customer logo, the shipped tab title and favicon, and the product default theme
instead of the workspace's. Sign in once and the branding appears from then on — including on the
sign-in screen after a sign-out. Reproduced on the ESR deployment: clear site data → neutral card;
sign in → "ESR / building intelligence", the ESR mark and favicon, and the `split` login layout the
brand selects; every later visit in that browser is branded.

Not fixable in the shell. Every `prefs.*` verb derives the workspace from the bearer token, so
pre-auth there is nothing to read a brand *from*: the login screen can only paint what this browser
already cached (`lb.brand.<ws>` in `localStorage`, written by the first authenticated
`prefs.resolve`). `resolveBootBrand` picks the best candidate out of that cache — URL `#/t/<ws>` hint,
then the typed email's last workspace, then the sole cached brand — and with an empty cache there is
no candidate, so it falls back to `DEFAULT_BRANDING`. Working as written; the first impression of a
branded deployment on any new device, new browser or cleared profile is simply the wrong brand.

The lb-side ask is therefore **one unauthenticated read**.

## What shipped

`GET /public/branding?ws=<ws>` — the gateway's **fifth** pre-auth route, beside `POST /auth/login`,
`POST /hooks/{ws}/{id}`, `POST /public/invite/accept` and `GET /public/invite/verify`. It returns the
workspace-default **`ui_branding`** and **`ui_theme`** blobs and nothing else.

- **`role/gateway/src/routes/public_branding.rs`** (new, 90 lines) — the handler. Reads
  `lb_prefs::get_workspace_prefs(store, ws)`, which touches *only* `workspace_prefs:[ws]` (the
  admin-owned workspace-default link), so there is no member record in reach even in principle.
- **`role/gateway/src/routes/rate_limit.rs`** — a second per-IP limiter (60/min, its own budget) and
  the `public_branding_rate_limit` middleware. The clock/key/429 body the two public middlewares
  shared is now one private `limit()` helper, so a third public route adds a limiter and a two-line
  wrapper rather than another copy.
- **`role/gateway/src/server.rs`** — the route, registered in the pre-auth block with the rationale.
- **`role/gateway/src/lib.rs`** — `BRANDING_MAX_AGE_SECS`, `PUBLIC_BRANDING_MAX_PER_WINDOW`,
  `PUBLIC_BRANDING_WINDOW_SECS` re-exported, so a test (and an embedder sizing an ingress cache)
  reads the contract from the crate instead of hardcoding it.

**Zero change to prefs, the host, capabilities, the store or the schema.** `ui_branding` and
`ui_theme` are shipped nullable axes on the closed `Prefs` record, admin-written through the existing
`prefs.set_default`; this route is a read of data that already existed.

## The four invariants

This is a deliberate, opt-in, read-only break in the workspace wall (§7), following the
document-store's public-serving precedent (README §6.12). Four properties keep it hairline; changing
any of them is a `/security-review` item, and each has a test that fails if it goes.

1. **The whitelist is construction, not filtering.** The handler destructures the loaded `Prefs` into
   two named fields and builds the body from those. The record is never serialized whole — contrast
   `prefs.get`, which does — so a *future* prefs axis cannot leak here by simply existing; someone
   would have to hand-add it to this file. The test sets **every other axis** on the record (language,
   timezone, date/time style, number format, unit system) and asserts each is absent from the raw body
   by name and by value.
2. **Not a workspace-existence oracle.** Unknown workspace, unbranded workspace, a slug the store's
   own guard rejects, missing `ws`, and a store error all return the byte-identical
   `200 {"ui_branding":null,"ui_theme":null}`. The test asserts the **bodies are equal**, not merely
   that the statuses match.
3. **`ws` is required and never inferred.** The tempting convenience — "a single-workspace node
   answers for its own workspace when `ws` is omitted" — was rejected: finding that workspace means
   enumerating workspaces for an anonymous caller, which is precisely what this route must not do. The
   sign-in screen always knows which workspace it is signing into, so it can always say so. A
   subdomain→workspace map, if one ever lands, resolves to a `ws` *in front of* this route.
4. **Rate-limited from day one**, per client key, with its own budget — a visitor repainting a login
   page must never spend the ceiling that protects the invite token/password oracle, or vice versa.

## Decisions worth the ink

- **`?ws=` query, not `/{ws}` path** (the issue proposed `/public/brand?ws=`; the scope sketched
  `/public/branding/{ws}`). A path segment makes a *missing* workspace a router `404` — a different
  answer from an unknown one, i.e. an oracle built by the routing table rather than the handler. The
  query form lets one code path answer every miss identically.
- **`branding`, not `brand`.** `brand.*` / `GET /brands` is already taken in this gateway by **report
  brand profiles** (`routes/brand.rs`), an unrelated feature. `/public/branding` matches the scope's
  name and the `ui_branding` axis it actually serves.
- **The theme rides along.** The issue asked for `ui_branding` *plus* `ui_theme`, and that is right:
  the ESR deployment's `split` login layout is selected by the **theme**, not the brand, so serving
  the brand alone would still paint the wrong login screen.
- **60/min, not the invite route's 10/min.** The brand read is neither a token nor a password oracle —
  it answers identically for every workspace — so the ceiling is not a brute-force ceiling; it is a
  cheap bound on scripted hammering of an unauthenticated store read. A sign-in paints the brand a
  handful of times, so 60/min is invisible to a human. Same `x-forwarded-for` keying, same "direct"
  shared bucket when unproxied (tighter, never looser).
- **`Cache-Control: public, max-age=60`, no etag.** Login is hot and the brand is stable, but a
  rebrand must show up on the next sign-in rather than being pinned for a session. The body is two
  small blobs, so a conditional round-trip would cost more than it saves.

## Tests (green)

`cargo test -p lb-role-gateway --test public_branding_route_test` — **5 passed**. Real node, real
store, real router; **no `Authorization` header anywhere** in the file.

- `serves_the_workspace_brand_with_no_token` — an admin sets the workspace default, an anonymous GET
  gets the brand + theme back, with the cache header.
- `serves_only_the_named_workspaces_brand` — **workspace isolation (mandatory)**: two real workspaces
  on one node; A's brand for A, B's for B, and A's response body carries no byte of B.
- `body_carries_the_brand_axes_and_nothing_else` — the wall-break guard (invariant 1): exact key set
  `{ui_branding, ui_theme}`, plus a raw-body scan for every non-brand axis on the record.
- `every_miss_answers_identically` — the oracle guard (invariant 2), five misses, equal bodies.
- `is_rate_limited_per_client` — the (MAX+1)-th read is 429 and a second client is untouched.

One test bug found and fixed while writing them: the first leak probe searched the raw body for the
*value* `"es"` (language) and tripped on `preset` — a two-letter code is a useless probe. It now scans
axis **names** plus distinctive values.

`cargo fmt --check -p lb-role-gateway` clean.

**Not run: the whole-crate `cargo test -p lb-role-gateway` sweep.** Two attempts died to the OOM
killer — `rust-lld invoked oom-killer` at 12:49 on this 14 GB machine, taking VS Code and Slack
helpers with it — because the crate links ~60 debug test binaries and cargo links many at once. The
operator's call was to stop building rather than retry under `-j 2`. So the **one unverified thing**
in this session is whether the shared `limit()` helper broke `invite_rate_limit_test`; the refactor is
a pure extraction (both middlewares call the same body they had inline), but it is unproven. Run
`cargo test -p lb-role-gateway --test invite_rate_limit_test -j 2` before this ships.

## Follow-ups

- **rubix-ai (the other half, not this repo):** `LoginView` fetches this keyed on the same hints
  `resolveBootBrand` uses today, with the `localStorage` cache as the instant first paint and the
  fetch as the correction; the boot script in `index.html` keeps painting the cached title/favicon so
  nothing regresses for a browser that has signed in before.
- **Blob size.** `ui_branding` carries small `*DataUri` marks and is capped client-side on write. The
  route serves whatever the admin stored, so an oversized blob is an oversized public response. The
  upload-side cap (scope: "Image size / format") remains the right place to bound it.
- The **logo/favicon-as-`assets.*`** half of the scope is still unbuilt — branding images ride inside
  the `ui_branding` blob as data URIs today, which is why this route needs no asset seam.
