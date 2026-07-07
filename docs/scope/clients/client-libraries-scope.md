# Clients scope — starter client libraries for the gateway surface

Status: scope (the ask). Promotes to `public/clients/` once shipped.

> Read with: `../auth-caps/api-keys-scope.md` (the bearer credential these clients
> present), `../ingest/ingest-scope.md` + `../ingest/webhooks-scope.md` (the
> `Sample` / series / webhook surface the libraries drive), `../mcp/mcp-scope.md`
> (the universal `POST /mcp/call` bridge), `../testing/testing-scope.md` §0 (the
> no-mocks rule these clients must obey), README §6.6 (auth), §7 (the workspace
> wall), §6.1 (the `series` / ingest surface).

We want **one small client library per language** (TypeScript/Node, Python, Go,
Rust) so an external caller — an appliance, a script, a backend service — can
**authenticate to a Lazybones node and reach the gateway surface in five lines**.
The libraries are deliberately **thin and incomplete**: each proves the
**authenticate → connect → round-trip a Sample** path in the language's idiom and
stops. The point is the shape to extend, not coverage — every verb stays reachable
through the universal `POST /mcp/call` bridge the client also exposes.

## Goals

- One folder per language under `clients/` — `node-ts/`, `python/`, `go/`, `rust/`
  — each self-contained, with its own manifest and README, **not** a member of the
  core `rust/` workspace or the `pnpm-workspace.yaml` (clients are external; they
  must never break the core build).
- A `Client` that takes a base URL + a bearer credential and **adds the
  `Authorization: Bearer …` header** to every call. The bearer is **either** an
  API key (`lbk_{ws}.{id}.{secret}`) **or** a JWT from `/login`; the client does
  not branch on which — the gateway already doesn't (it splits on the `lbk_`
  prefix in one chokepoint).
- A `login()` helper that calls `POST /login` with `{user, workspace}` and stores
  the returned dev token — the **local-dev** path (`make cloud` + login as
  `user:ada`).
- The two calls that prove the loop end to end, both **typed** in the language:
  - `writeSamples(samples)` → `POST /ingest` (the durable write surface).
  - `latestSample(series)` → `GET /series/{series}/latest` (the read-back).
- A `callMcp(tool, args)` helper → `POST /mcp/call` — the universal escape hatch.
  Anything else the platform does is reachable from here without a library update.
- A webhook helper for the **third-party caller** path: `signWebhook(secret, body)`
  (HMAC-SHA256 → `sha256=<hex>`) and `postWebhook(url, headers, body)`. This is
  the path a non-member external service uses (no API key of its own; it signs).
- A runnable example per language that does: `login` (or read a key from env) →
  `writeSamples` → `latestSample` → print. Hit a real `make cloud` node.

## Non-goals

- **No full SDK.** No typed wrapper for every verb (`channels.*`, `flows.*`,
  `inbox.*`, …) — those stay one `callMcp` away. Wrapping them is a per-caller
  choice; the library does not assume which surface the caller needs.
- **No retry/backoff, no pagination iterator, no streaming/SSE helper** in v1.
  Each is its own design decision (cursor codec, job watcher, EventSource
  reconnection) and adding them here would lock the shape prematurely.
- **No code generation from an OpenAPI spec.** The gateway has no OpenAPI
  contract today; hand-writing the 3–4 routes keeps the surface honest and the
  libraries dependency-free.
- **No bundler, no transpile pipeline.** Each library uses the language's native
  build (`tsc`, `go build`, `cargo build`, plain Python) — nothing to install
  before reading the source.
- **No new core verbs, caps, routes, or tables.** This is glue over the shipped
  surface; the platform side is unchanged.
- **No publish to a registry (npm / PyPI / crates.io / pkg.go.dev) in v1.** The
  README names this as the follow-up; the source is the contract today.

## Intent / approach

**One idiomatic thin client per language, all sharing the same five-method
shape**, each in a folder under `clients/`. The shared shape is the contract; the
idioms differ (Promises vs `async def` vs channels vs `Future`). Each library has
exactly these files (FILE-LAYOUT: one verb per file, ≤150 lines):

```
clients/<lang>/
  README.md           — install, auth (both kinds), the round-trip example
  <manifest>          — package.json | pyproject.toml | go.mod | Cargo.toml
  src/
    client.<ext>      — the Client: config + bearer header + login()
    ingest.<ext>      — writeSamples() + latestSample()
    mcp.<ext>         — callMcp() (the universal bridge)
    webhook.<ext>     — signWebhook() + postWebhook() (third-party path)
    index.<ext>       — barrel re-export only
  example.<ext>       — the runnable login → write → read demo
```

**Why a per-language folder and not a polyglot single library:** each ecosystem
has a native packaging/distribution story (npm, PyPI, crates.io, pkg.go.dev) and
a native HTTP + crypto story. Pretending they share a tree (codegen, or a
"base + bindings") forces one ecosystem's constraints on the others and creates a
build dependency the platform doesn't need. Five files per language is cheap; one
wrong abstraction is expensive.

**Why the libraries live in the repo (not a separate `lb-clients` repo) for v1:**
keeps the wire contract in lockstep with the gateway code — a route change can
update the clients in the same PR. The cost (a reader thinks the clients are
core) is paid down by (a) the `clients/` folder being outside the workspaces, (b)
the README saying "external", and (c) the non-goal on registry publication
already implying "extract the day a v1 shape is agreed". Splitting earlier
(=before the shape is proven) just means a cross-repo dance for every wire tweak.

> *Rejected: a single OpenAPI-generated multi-language SDK.* The gateway is
> hand-routed (`server.rs`), so the OpenAPI spec would itself become an artifact
> we maintain by hand — duplicating the routes in a second place. Until the
> gateway is spec-first (not planned), five hand-written files per language wins.

## How it fits the core

- **Tenancy / isolation:** the client **never** sends the workspace on a request
  body. The workspace is encoded in the bearer (the `lbk_{ws}…` prefix for a key,
  the JWT claim for a dev token) and the gateway is the only thing that reads it.
  This is the same hard wall the UI obeys (§7); the client mirrors it so a caller
  cannot accidentally send `ws` in the body and expect it to be honored.
- **Capabilities:** the libraries hold **no cap logic**. They present the bearer;
  the gateway resolves caps from the API-key record or the JWT and gates each
  verb. A `writeSamples` against a key lacking `mcp:ingest.write:call` returns
  `403` (opaque) — the client surfaces it as an error, never tries to "fix" it.
- **Placement:** N/A — these are caller-side libraries; they reach any node
  (edge or cloud) whose gateway address the caller points them at.
- **MCP surface:** the libraries are thin REST/MCP callers. They expose:
  - **CRUD:** none — the write path is `POST /ingest`, which is an *append*, not
    a CRUD verb over a resource the client owns. (Ingest's CRUD-ish reads are
    `GET /series/*`, surfaced as `latestSample`. The full read set — list, range,
    find — is one `callMcp` away and intentionally not wrapped in v1.)
  - **Get / list:** `latestSample(series)` only — the one read that proves the
    round-trip. `series.list` / `series.read` / `series.find` are deliberate
    non-goals for v1; reachable via `callMcp`.
  - **Live feed:** N/A — no SSE helper in v1 (non-goal). The `series.read` +
    polling, or a future `EventSource` wrapper, is the caller's call.
  - **Batch:** `POST /ingest` already takes a `Sample[]`; the library passes the
    array through. The "batch that can run long must be a job" rule (§6.1) does
    not apply — ingest staging is bounded and always-fast.
- **Data (SurrealDB):** the libraries touch no datastore — they are HTTP clients.
- **Bus (Zenoh):** N/A — the libraries are state-plane callers; they do not
  publish or subscribe to motion. (A future SSE helper rides the gateway's
  existing `/series/{s}/stream`, not Zenoh directly.)
- **Sync / authority:** N/A — caller-side.
- **Secrets:** the libraries hold a bearer string in memory and never persist it.
  The READMEs are explicit: **read the API key from an env var, do not hard-code
  it, and never log the bearer.** This is the same discipline as the in-repo CLI
  (`cli/operator-cli-scope.md`).

## Example flow

A script that wants to push a metric and read it back, in each of the four
languages (identical shape, idiomatic syntax):

1. **Get a bearer.** Either:
   - mint an API key once via the admin console (or `POST /admin/apikeys` with a
     session token) and read it from an env var — for a long-lived producer, OR
   - call `client.login(user, workspace)` — for a dev/admin script.
2. `client.writeSamples([{series, ts, seq, payload, labels}])` →
   `POST /ingest` → `{accepted, committed}`.
3. `client.latestSample(series)` → `GET /series/{series}/latest` →
   `{sample: {…, payload}}` — the round-trip is proven.

A **third-party webhook sender** (no key of its own; the admin shared a secret):

1. `signature = client.signWebhook(sharedSecret, rawBody)` — HMAC-SHA256, output
   `sha256=<hex>` (the format the gateway's `signature` mode expects).
2. `client.postWebhook(url, {"X-Signature": signature}, rawBody)` →
   `POST /hooks/{ws}/{id}` → `202 {id, series, seq}`.

## Testing plan

The libraries must compile (the cheap gate) AND a round-trip must pass against
a **real node** (rule 9 — no `*.fake.ts` re-implementing the gateway). Each
language folder ships:

- **Static check (mandatory for all four):** `cargo build` / `tsc --noEmit` /
  `go build ./...` / `python -c "import lb_client"`. The README documents it.
- **Live round-trip (mandatory for `rust/` and `node-ts/`, optional for `go/` and
  `python/` in v1):** a test/example that boots the real in-process gateway (the
  `test_gateway` bin pattern from `rust/role/gateway`), calls `writeSamples` +
  `latestSample`, and asserts the payload round-trips. `clients/rust` reuses the
  gateway's test infra (it is the one language that can pull the gateway crate
  directly). `clients/node-ts` documents the `pnpm test:gateway` pattern
  (`vitest.gateway.config.ts` against a real node), as the UI already does.
- **Capability-deny (mandatory):** the round-trip test must include one call
  that gets `403` (a key/token missing `mcp:ingest.write:call`) and assert the
  client surfaces it as an error — never silently swallows. This is the same
  mandatory category every other scope satisfies.
- **Workspace-isolation:** the `rust/` test mints a key in ws-A and asserts it
  cannot read or write ws-B; the client surfaces the `403`/`404` opaquely.

No mocks: the gateway is real in every test that asserts behavior. A test that
only checks the URL/header construction may be a pure unit test (no gateway) —
that is the **only** layer allowed to run without the real node.

## Risks & hard problems

- **Drift from the wire.** Five hand-written files per language × four languages
  = four places to update when a route changes. Mitigation: tiny surface (3
  routes + MCP bridge), the `clients/<lang>/README.md` is the per-language
  contract, and the canonical reference lives once in `skills/ingest-series/` +
  this scope. The day the gateway becomes spec-first, regenerate.
- **Bearer-handling mistakes.** A library that logs the request headers, or
  inlines the key into a URL, leaks the credential. Mitigation: the README
  states the discipline; `request()` logs only the status + path; the bearer
  lives in a `Authorization` header the caller cannot accidentally URL-encode.
- **Silent cap-deny.** The most common client bug is "the call returns 403 and
  the library throws a generic error". Mitigation: each client's error type
  carries the status + body so the caller can see "denied" and act on it. The
  capability-deny test pins this.
- **Webhook raw-body requirement.** The single most common webhook bug is HMAC
  over a re-serialized body (the gateway pins this in
  `webhook_routes_test.rs::signature_mode_body_tamper_breaks_signature`). The
  `signWebhook` helper must take `bytes`/`[]byte`/`Uint8Array`/`Vec<u8>` — never
  a string — and the README says so. A test signs the original bytes and posts
  re-serialized bytes with the same JSON value; it must fail.

## Open questions

- **Distribution.** When does each library publish (npm/PyPI/crates.io/
  pkg.go.dev), and under what name (`@lazybones/client-*`? `lb-client-*`?) —
  decided the day the v1 shape is signed off and a real external consumer exists.
  Until then the README says "vendor / git-submodule / `cargo path = { … }`".
- **Streaming.** Is `EventSource`/`GET /series/{s}/stream` a v2 addition (one
  helper that takes an `onSample` callback) or stays caller-side forever? Defer
  until a caller actually needs live motion instead of poll-then-read.
- **Auth helper ergonomics.** Should `login()` cache + auto-refresh the token
  near expiry, or is "caller's problem" the right v1 (the dev token lasts 12h;
  the appliance key never expires)? v1 keeps it caller-side.
- **A `make client-roundtrip` target.** Should the root Makefile gain a target
  that runs all four round-trips against `make cloud`? Likely yes once `go/` and
  `python/` round-trip tests exist.

## Related

- README §6.6 (identity/auth/caps), §7 (the workspace wall), §6.1 (ingest/
  series), §6.13 (the SSE surface a future streaming helper would ride).
- Auth path: `docs/scope/auth-caps/api-keys-scope.md`,
  `docs/public/auth-caps/auth-caps.md`, source `rust/crates/apikey/`,
  `rust/role/gateway/src/session/authenticate.rs`.
- Ingest path: `docs/scope/ingest/ingest-scope.md`, `docs/public/ingest/ingest.md`,
  `docs/skills/ingest-series/SKILL.md`, source `rust/crates/ingest/`,
  routes `rust/role/gateway/src/routes/ingest.rs`.
- Webhook path: `docs/scope/ingest/webhooks-scope.md`,
  `docs/public/ingest/webhooks.md`, source `rust/role/gateway/src/routes/webhook.rs`,
  `rust/role/gateway/src/routes/admin_webhooks.rs`.
- MCP bridge: `docs/scope/mcp/`, route `rust/role/gateway/src/routes/mcp.rs`.
- Sibling skill (the canonical wire reference): `docs/skills/ingest-series/SKILL.md`.
- The in-repo CLI (the fourth client of the gateway, same posture):
  `docs/scope/cli/operator-cli-scope.md`.
