# Session: gateway `/health` route (issue #72)

## The ask

Issue [#72](https://github.com/NubeDev/lb/issues/72): the gateway serves no health route, so
nothing an LB/orchestrator can probe answers "is this node up?" without a session token. Build
`GET /health` against the fleet contract already decided in
[`scope/deploy/containerize-scope.md`](../../scope/deploy/containerize-scope.md) §"The health
contract". Stay on `master`; commit no code.

## What shipped (uncommitted, on `master`)

- **`rust/role/gateway/src/routes/health.rs`** (new) — the `GET /health` handler + the
  `HealthGate` in-memory cell (one `AtomicBool` per subsystem: `store`, `gateway`) + the
  `HealthBody`/`HealthDetail` response types + the compiled-in `VERSION`. The handler loads two
  atomics, maps serving ⇒ `200` / any-degraded ⇒ `503`, and emits exactly the contract body.
- **`rust/role/gateway/src/state.rs`** — `Gateway` gains a `pub health: SharedHealthGate` field
  (`Arc<HealthGate>`), initialised serving in `Gateway::build`. Axum clones the `Arc` cheaply per
  request.
- **`rust/role/gateway/src/routes/mod.rs`** — `mod health;` + `pub use health::{health, HealthGate,
  SharedHealthGate};`.
- **`rust/role/gateway/src/server.rs`** — `.route("/health", get(health))` registered **first**
  in the router, beside the other unauthenticated routes, with a doc block recording the contract.
- **`rust/role/gateway/tests/health_route_test.rs`** (new) — 6 tests over the real gateway.
- **Docs** — this session, [`scope/deploy/health-route-scope.md`](../../scope/deploy/health-route-scope.md),
  and the STATUS / scope-README index updates.

## Intent held to (the two load-bearing calls)

1. **No store ping in the handler.** The contract's sharpest rule is "a health check that can hang
   is a health check that lies", and `store.query` would both hang-risk and contend for the global
   session mutex with real work. The handler reads two `AtomicBool`s, nothing else. Both default to
   serving — the honest answer at this layer (the store handle is alive once `Node::boot` opened it,
   and the gateway is constructed after, so the handle exists for every probe the route can ever
   serve; `system-map-scope.md` already says "the handle exists" is not real liveness, and this
   route does not pretend otherwise).
2. **The 503 path is a real seam, not faked.** The contract mandates both states be producible.
   `HealthGate::set_store`/`set_gateway` are the attachment point a FUTURE in-process monitor (a
   store-down detector, a drain-on-shutdown handoff) flips; no caller flips them today, and the
   scope doc says so plainly rather than dressing always-200 as detection.

## Decisions resolved during the session

- **`/health`, never `/healthz`.** Held — the `z` is a k8s/Borg namespace-collision device; we ship
  no k8s. A test asserts `/healthz` (and `/livez`/`/readyz`/`/startupz`/`/api/health`) all 404.
- **Version field = `env!("CARGO_PKG_VERSION")` of the gateway crate** (workspace `0.1.0`), NOT the
  embedding product's version. An LB pinning a matcher wants "which lb-gateway build is running";
  embedders bump the lb tag they pin. Matches the contract example.
- **No `BootConfig` field.** The issue names this explicitly. `HealthGate` lives on `Gateway`
  (crate-internal, `Arc`-shared); if a future monitor needs to drive it from outside the gateway
  crate, re-open then.
- **Route registered FIRST, outside the auth wall.** Beside `/login`/`/hooks`/`/public/invite/*`.
  A test proves a garbage `Bearer` header returns the same 200 (the route never reaches the auth
  wall).

## Testing (real, no mocks — rule 9)

`cargo test -p lb-role-gateway --test health_route_test` — **6/6 green**:

```
health_503_when_a_subsystem_is_degraded ... ok
health_503_when_the_gateway_subsystem_is_degraded ... ok
health_a_stale_bearer_does_not_change_the_answer ... ok
health_ok_unauthenticated_with_version_and_detail ... ok
health_returns_to_ok_after_a_degrade_is_cleared ... ok
healthz_is_not_registered ... ok
```

The mandatory capability-deny + workspace-isolation categories do **not** apply (the route is
unauthenticated and workspace-agnostic — there is no principal to deny and no workspace to isolate);
that is asserted by the "no Authorization header" and "garbage bearer" cases.

**No regressions:** `gateway_test` (9), `gateway_routes_test` (8), `login_hardening_test` (3) all
green after the new `Gateway.health` field. `cargo build -p lb-role-gateway` clean;
`cargo fmt -p lb-role-gateway --check` clean; `cargo clippy -p lb-role-gateway --lib` clean on the
new files. (A pre-existing `clippy::min_max` deny in `crates/frame/src/group.rs:143` blocks
workspace-wide `--all-targets` clippy; it is unrelated to this change — `git diff` confirms
`crates/frame/` untouched.)

## Non-goals held

- No real store-down detector (the seam ships empty-but-honest).
- No product-host bundle change in this repo (their `tcp:` → `http:` flip is their own repo).
- No `/livez`/`/readyz`/`/startupz`.

## Follow-ups (named, not done)

- Flip `fly.toml`'s `[[http_service.checks]]` from `GET /` to `GET /health` (matcher `200`) in the
  fly-deploy assets — out of this session's scope, recorded in the scope doc's Example flow.
- Drive `HealthGate` from a real monitor when one exists (store-down detection / drain-on-shutdown).
- `rubix-ai`/`ems-node` bundle health specs flip `tcp:` → `http:` in those repos once they bump the
  lb tag carrying this route.
