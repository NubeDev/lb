# deploy scope — the gateway `/health` route

Status: **scope (the ask)** — issue [#72](https://github.com/NubeDev/lb/issues/72). The contract
is already decided fleet-wide in [`containerize-scope.md`](containerize-scope.md) §"The health
contract"; this is the gateway's implementation of it. Promotes to `doc-site/content/public/deploy/`
once shipped.

The gateway serves **no health route**, so nothing can ask "is this node up?" without authenticating
and calling a real verb. Every embedder (`rubix-ai`, `ems-node`, `rartifacts`) inherits the gap:
`fly-deploy` and `containerize` both had to probe `GET /` instead and record it as a known
concession, and `rubixd`'s rollback-health gate can only fall back to `tcp:<port>` against a product
host (which proves a socket accepts, not that the node works). Verified against a live `rubix-ai`
node (`node-v0.4.5`): `GET /health`, `/healthz`, `/api/health` all `404`.

## Goals

- **One unauthenticated `GET /health` route on the gateway port.** An LB/orchestrator has no bearer
  token; it sits outside the auth wall like `POST /login`.
- **The fleet contract body, verbatim:** `200 {"status":"ok","version":…,"detail":{…}}` serving;
  `503 {"status":"degraded",…}` alive-but-not-serving. `/health`, **never `/healthz`**; one route,
  no `/livez`/`/readyz`/`/startupz`.
- **Reads in-memory state only** — no store query, no disk I/O, no network call. A health check that
  can block on a dependency is a health check that can lie.
- **Leaks nothing** beyond `status` + `version`; `detail` names *which* subsystem is degraded,
  never a path, DSN, or key.
- **Always on when `GatewayMode::Addr`** — embedders need no `BootConfig` field for it.

## Non-goals

- **No real liveness probe.** "The handle exists" is not real liveness
  (`../system-map/system-map-scope.md` already says so); this route does not pretend it is by pinging
  the store. The contract is deliberately shallow — `detail` reports the in-memory posture, nothing
  more.
- **No store-down detector today.** The 503 path is a seam, wired but not yet driven (see Intent).
- **No `/healthz`.** Resolved: the `z` suffix is a k8s/Borg namespace-collision device; we ship no
  k8s and have no collision.
- **No product-host (`rubix-ai`/`ems-node`) bundle change in this repo.** Once this lands their
  health specs flip `tcp:` → `http:` in their own repos; the fleet plane needs no change
  ([`rubixd-rartifacts-scope.md`](rubixd-rartifacts-scope.md) §Open questions records this as decided).

## Intent / approach

A single axum route, `GET /health`, registered **first** in the router so it is reachable at a
stable path with zero auth machinery in front of it, beside the other unauthenticated routes
(`/login`, `/hooks`, `/public/invite/*`). The handler reads one in-memory cell — a `HealthGate`
(`Arc`-shared, one `AtomicBool` per subsystem the contract names: `store`, `gateway`) — and maps the
load to the `200`/`503` + body. Load-only atomics ⇒ the probe can never block on a dependency.

**Why a gate and not a store ping.** The contract's one 503 trigger named is "store is not open".
But querying the store from `/health` is forbidden (it can hang; and `use_ns` would contend with
real work). At this layer the honest signal is: the store handle is alive (it is — `Node::boot`
opened it before the gateway was constructed, so the handle exists for every probe the route can
ever serve) and the gateway is tautologically ok while handling. So both subsystems default to
serving and the route answers `200` — the truthful answer today. The per-subsystem setters
(`HealthGate::set_store` / `set_gateway`) are the seam a **future** in-process monitor flips (a
store-down detector, a drain-on-shutdown handoff) without the route shape changing; no caller flips
them yet. **Rejected:** a live `store.query` in the handler (can hang, contends for the session
mutex); a second admin listener (a new surface to secure, for no gain); `/healthz` + `/readyz`
(ratified against — see Non-goals).

**Version field.** `env!("CARGO_PKG_VERSION")` compiled into the gateway crate — a stable identifier
for "which lb-gateway build is running" that an LB can pin a matcher on. Matches the `version` field
the contract documents.

## How it fits the core

- **Tenancy / isolation:** none — the route is workspace-agnostic and carries no workspace context.
  It sits outside the auth wall; it never reads a token, so the workspace wall is not in play.
- **Capabilities:** none — unauthenticated by design (an LB has no bearer), same posture as
  `/login`. It gates nothing and leaks no existence information beyond "the process answers".
- **Placement:** either role — it is a gateway-route concern, and the gateway is config-attached to
  any node (`GatewayMode::Addr`). No `if cloud` (rule 1).
- **MCP surface:** none. It is not an MCP verb and is not in the catalog; it is an HTTP ops route.
- **Data (SurrealDB):** none — reads in-memory atomics only, by contract.
- **Bus (Zenoh):** none.
- **Sync / authority:** node-local; each node answers for itself.
- **Secrets:** none — leaks only `status` + `version` + subsystem names.

## Example flow

1. An ALB target-group health check (or `fly.toml`'s `[[http_service.checks]]`, or `rubixd`'s
   rollback gate) issues `GET /health` against the node's gateway port with no credentials.
2. The handler loads the two `HealthGate` atomics. Both serving ⇒ `200
   {"status":"ok","version":"0.1.0","detail":{"store":"ok","gateway":"ok"}}`. The ALB keeps the
   target; `rubixd` commits the staged release.
3. A future store-down monitor flips `HealthGate::set_store(false)` in-process. The next probe loads
   it ⇒ `503 {"status":"degraded","version":"…","detail":{"store":"degraded","gateway":"ok"}}`. The
   ALB de-registers the target (non-200) WITHOUT the restart-on-connection-failure supervisor
   touching it (the process still answers). When the monitor clears the flag, the next probe is 200
   again.
4. A dead node: the socket refuses ⇒ the ALB/supervisor treats connection-refused as dead and
   restarts it. That path is unchanged by this route.

## Testing plan

Real gateway over a real booted node (rule 9 — no fake backend), in
`rust/role/gateway/tests/health_route_test.rs`:

- **200 shape + open auth:** bare `GET /health` with **no** `Authorization` header ⇒ `200`, body is
  exactly `{status:"ok", version, detail:{store:"ok",gateway:"ok"}}`.
- **Leaks nothing:** assert the top-level key set is exactly `{status,version,detail}`, `detail` is
  exactly `{store,gateway}`, and every detail value is `"ok"|"degraded"` (never a path/DSN/key).
- **Stale/garbage bearer is a no-op:** the route never reaches the auth wall, so a `Bearer
  not-a-real-token` header returns the same 200 (not 401).
- **`/healthz` not registered:** `/healthz` ⇒ 404; also `/livez`/`/readyz`/`/startupz`/`/api/health`
  ⇒ 404 (the k8s-isms and the LB-common misspellings all absent).
- **503 degraded path:** flip `HealthGate::set_store(false)` (in-memory, no store call) ⇒ `503`,
  `status:"degraded"`, `detail.store:"degraded"`, `detail.gateway:"ok"`. Symmetric for
  `set_gateway(false)`.
- **Recovery:** after a `set_store(false)` → `set_store(true)`, the next probe is `200` again.

**Capability-deny / workspace-isolation (the mandatory categories) do not apply** — the route is
unauthenticated and workspace-agnostic by design (there is no principal to deny and no workspace to
isolate). That is asserted by the "open auth" and "stale bearer" cases above.

## Risks & hard problems

- **Pretending to be a real liveness probe.** The temptation is to `store.query` to "really" check.
  That is the trap: it can hang (the contract's sharpest rule) and it contends for the global
  session mutex with real work. The honest answer today is "the handle is alive"; the 503 seam is
  reserved for a real in-process signal, not faked by a probe.
- **The 503 seam looking dead.** Two always-true atomics read suspiciously like cargo code. The
  gate is the contract-mandated shape (both states must be producible); it is *honest* precisely
  because it does not fake a store-down detection that does not exist. The setters are documented as
  the future-monitor attachment point.
- **Version drift across embedders.** `env!("CARGO_PKG_VERSION")` is the gateway crate's version
  (workspace), not the embedding product's (`rubix-ai`'s `node-v0.4.5` is its own version). That is
  the right thing: an LB pinning a matcher wants "which lb-gateway build", and embedders bump the lb
  tag they pin.

## Open questions

- **Does any embedder need to force-degrade?** Resolved as **no, not today**: `HealthGate` lives on
  `Gateway` (crate-internal, `Arc`-shared); an embedder reaching `lb_role_gateway::Gateway` could
  flip it, but no `BootConfig` field is added (the issue names this — "embedders need no BootConfig
  field for this"). Re-open if a real monitor needs to drive it from outside the gateway crate.

## Related

- [`containerize-scope.md`](containerize-scope.md) §"The health contract" — the ratified contract.
- [`rubixd/token-auth-scope.md`](rubixd/token-auth-scope.md) + [`rartifacts/server-core-scope.md`](rartifacts/server-core-scope.md) — the fleet binaries' `/health` implementations of the same contract.
- [`fly-deploy-scope.md`](fly-deploy-scope.md) §"No unauthenticated `/health` route" — the recorded concession this closes.
- Issue [#72](https://github.com/NubeDev/lb/issues/72).
