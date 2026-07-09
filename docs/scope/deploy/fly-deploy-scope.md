# Deploy scope — Fly.io (single-container node) with reusable Docker/CI assets

Status: scope (the ask). Promotes to `doc-site/content/public/deploy/` once shipped.

We want a **one-command deploy of a Lazybones `node` to Fly.io** — a single Fly Machine
running the gateway-mounted node behind Caddy, with the React SPA served from the same
origin, a persistent volume for the embedded SurrealDB store, and the **federation
sidecar wired for SQLite datasources** (the Docker-free `demo-buildings.db`
pre-registered). The build inputs — Dockerfile, Caddyfile, entrypoint, ignorefile —
live in `deploy/common/` and are **shared, verbatim, across
local Docker, GitHub CI, and the Fly deploy**, so the image you run locally is the image
Fly runs and the image CI builds. Modeled on the battle-tested `dev-pulse` Fly runbook,
minus its bundled Postgres — Lazybones embeds its one datastore, so there is **no
separate DB service**.

## Goals

- `make fly-deploy` ships the current tree to a Fly app in one command (remote builder,
  parent-dir-free context, no local Docker daemon needed).
- **One image, many drivers.** The Dockerfile + entrypoint + Caddyfile in `deploy/common/`
  are the single source of truth. Local `docker compose`, the GitHub CI build job, and
  `fly deploy` all point at the *same* files — no per-target Dockerfile drift.
- **Same ignore file.** One `.dockerignore` content, referenced by every build path
  (mirroring dev-pulse's `.dockerignore` ↔ `.dockerignore.fly` pairing). A build never
  balloons the context differently depending on who invoked it.
- The deployed node is a **real, standalone node**: login, MCP, SSE, agents, flows,
  rules, insights — the `cloud` posture (`LB_GATEWAY_ADDR` set), persisted across
  restarts on a Fly volume.
- **Datasources work end to end** on Fly via the federation sidecar in **SQLite mode**:
  `net:*` pre-approved for the `127.0.0.1:0` sqlite convention, and the shipped
  `demo-buildings.db` seeded + registered so the Datasources page probes green and Data
  Studio can query it on first boot. **No hosted Postgres.**
- A local **HTTPS pre-flight** (mkcert + Caddy) so a deploy iteration is proven in ~30 s
  locally before the ~5–10 min Fly round-trip — the single biggest time-saver dev-pulse
  documents.

## Non-goals

- **No hosted/managed Postgres, and no bundled Postgres-in-the-image.** Lazybones embeds
  SurrealDB (rule 2); external SQL is reached only through the federation sidecar as a
  *federated source*, never a second authority. Postgres/Timescale sources remain
  supported by the sidecar, but the deploy defaults to and is documented for **SQLite**.
- No AWS / other cloud targets. (`deploy/aws/` was an example and is removed.)
- No multi-Machine / HA / horizontal scale. One Machine, one volume. Multi-node sync is
  `sync/`, orthogonal to this.
- No custom auth provider wiring (OAuth apps, GitHub PATs) — the node's own login gate
  and API keys (`auth-caps/`) govern access; this scope is *deployment plumbing only*.
- No changes to any core crate or UI product code. This is toolchain + config + docs.
- Desktop/Tauri packaging is `desktop/` — a windowed app, not a container workload.

## Intent / approach

**One container, three processes under `tini`** (the dev-pulse topology, minus PG):

| # | Process | Bind | Role |
|---|---|---|---|
| 1 | **Caddy 2** | `0.0.0.0:8080` | Serves the SPA bundle (`/`, `/assets/*`, static roots) and reverse-proxies **everything else** to the node. `auto_https off` — fly-proxy terminates TLS. |
| 2 | **node** (`LB_GATEWAY_ADDR=127.0.0.1:8731`) | `127.0.0.1:8731` | The gateway-mounted Lazybones node. Owns the embedded SurrealDB store on `/data`, runs the boot seeders and reactors. |
| 3 | **federation sidecar** | `127.0.0.1:*` | Supervised by the node (`LB_FEDERATION_ENDPOINTS` set), reachable only through the host. SQLite sources via the `127.0.0.1:0` convention. |

fly-proxy terminates TLS on 80/443 → plain HTTP to Caddy `:8080`. Caddy is **not** the
TLS terminator. The SPA is a **browser build with `VITE_GATEWAY_URL=""`** so every API
call is **same-origin relative** (`/login`, `/mcp/call`, `/…/stream`) — Caddy proxies
those to `127.0.0.1:8731`. This is why Caddy can be "SPA = explicit static paths;
everything else = backend": the app and the gateway share one origin.

```
                    https://<app>.fly.dev/
                              │  fly-proxy (TLS)
                    ┌─────────▼──────────────┐
                    │ Caddy  :8080           │
                    │  /,/index.html,/assets │ → SPA bundle (embedded in image)
                    │  everything else       │ → 127.0.0.1:8731
                    └─────────┬──────────────┘
                    ┌─────────▼──────────────┐
                    │ node (gateway)         │
                    │  127.0.0.1:8731        │
                    │  store → /data/store   │──┐
                    └─────────┬──────────────┘  │  one Fly volume `lb_data`
                    ┌─────────▼──────────────┐  │      mounted at /data
                    │ federation sidecar     │  │
                    │  sqlite: /data/demo/*.db│◄─┘
                    └────────────────────────┘
```

**Reuse mechanism (the load-bearing decision).** Build inputs live once in
`deploy/common/`:

- `deploy/common/Dockerfile` — multi-stage: Rust release build of `node` + the
  `federation` sidecar (postgres feature) + the wasm guests → pnpm SPA build
  (`VITE_GATEWAY_URL=""`) → debian-slim runtime with Caddy + tini + the two binaries +
  the SPA `dist/`.
- `deploy/common/Caddyfile` — SPA-explicit + allow-all reverse proxy (the dev-pulse
  "handle blocks, first-match-wins" pattern that avoids the `try_files`-eats-the-API bug).
- `deploy/common/entrypoint.sh` — ensure `/data` dirs → start Caddy → start the node → seed the
  sqlite demo file if absent → wait on the node. **Discovered during implementation:** the node
  is entirely env-driven (no `config.toml` — see `rust/node/src/main.rs`/`federation.rs`), so
  there is no template to `envsubst`; the entrypoint just sets env vars directly. Simpler than
  the dev-pulse precedent, not a config-template port.
- **The repo-root `.dockerignore`** (NOT a `deploy/common/` copy) — **discovered during
  implementation:** Docker resolves `.dockerignore` from the *context root*, never next to the
  Dockerfile; a per-Dockerfile ignore file only works with `buildx build --ignorefile`, which
  local `docker compose build` and CI's classic `docker build` don't use. So there is exactly
  ONE ignore file for every repo-root-context build (desktop's included), denylist-style, with a
  narrow `!docker/postgres/seed*.py` etc. carve-out for the demo-dataset generator this deploy
  needs. The originally-planned separate `deploy/common/.dockerignore` (a whitelist) would have
  been silently ignored by two of the three drivers — the "same ignore file everywhere" goal
  means literally the same *file*, not a same-content copy.

Then each **driver** is a thin pointer, not a fork:

- **Fly** (`deploy/fly/fly.toml`) — `[build]` names `deploy/common/Dockerfile`;
  `make fly-deploy` passes `--dockerfile deploy/common/Dockerfile --ignorefile .dockerignore`
  (Fly's remote builder uses buildx, so this flag IS honored there). `[mounts]` → `/data`.
  `[http_service]` → `:8080`.
- **Local compose** (`deploy/common/compose.yml`) — same Dockerfile, a named volume for
  `/data`, plain HTTP on a host port.
- **Local HTTPS pre-flight** (`deploy/fly/compose.fly-local.yml` +
  `deploy/fly/Caddyfile.local`) — the same image behind mkcert TLS on `https://localhost`.
- **CI** (`.github/workflows/ci.yml`, a new `deploy-image` job) — `docker build -f
  deploy/common/Dockerfile` to prove the image builds on every PR (build-only; no push,
  no deploy from CI in v1).

**Why one Fly Machine with an embedded store (not a DB service).** Rule 2: SurrealDB is
embedded on every node — there is nothing to attach. This makes the Lazybones deploy
*simpler* than dev-pulse's: no `initdb`, no `pg_ctl`, no `DATABASE_URL`. The only durable
state is the SurrealKV directory + the sqlite demo file, both on the one `/data` volume.
**Alternative rejected:** a separate managed DB — impossible by rule 2 and pointless
(the store is in-process).

**Why SQLite for datasources (not Postgres) on Fly.** The federation sidecar already
ships a first-class `sqlite` kind (`sqlite-datasource-demo-scope.md`); a sqlite source is
a node-local **file** with no network endpoint, so it needs no second container, no
network grant beyond the `127.0.0.1:0` convention, and no secret DSN. It rides the same
`/data` volume. Postgres/Timescale sources still work (point `LB_FEDERATION_ENDPOINTS` +
a seed DSN at an external host), but they are **not** part of the default deploy.

## How it fits the core

- **Symmetric nodes (rule 1):** the Fly node is the **exact same binary** as `make dev`,
  selected into the cloud posture purely by config (`LB_GATEWAY_ADDR` set). No
  `if cloud`. The Dockerfile builds the one `node`; the entrypoint sets env, nothing more.
- **One datastore (rule 2):** SurrealDB embedded, persisted at `/data/store`. No PG, no
  blob service. External SQL is federated, never authoritative.
- **Workspace is the hard wall (rule 6):** unchanged — the deploy serves the same
  gateway; every request derives ws + principal from the token. The deploy adds no seam
  that bypasses this. `LB_WORKSPACE` picks the boot/seed workspace, as in dev.
- **Capability-first (rule 5):** unchanged. The federation sidecar's `net:*` grant for
  the sqlite convention is the **same admin-approved grant** `make dev` uses
  (`LB_FEDERATION_ENDPOINTS=…,127.0.0.1:0`) — no pre-approval bypass, no new authority.
- **Core knows no extension (rule 10):** the deploy wires the **federation** sidecar the
  same generic way `node/src/federation.rs` already does — env-keyed, opaque id. No core
  file learns "fly". The deploy dir names the extension; that is binary-boundary config,
  not a core branch.
- **Placement:** cloud-only *posture*, but not a code branch — it is one value of
  `LB_GATEWAY_ADDR`. The same assets can run an edge appliance headless (drop the env).
- **MCP surface:** **none added.** This scope introduces no MCP verb, capability, route,
  or table. Datasources register through the already-shipped `datasource.add` /
  `federation.*` verbs; the deploy just seeds one via `seed-demo-sqlite.sh`.
- **Data (SurrealDB):** no schema change. The only new persistent artifacts are the store
  directory and the demo sqlite file on the volume.
- **Bus (Zenoh):** unchanged; motion stays in-process on the single node.
- **Secrets:** the gateway signing key (`LB_SIGNING_KEY`) and any model/API keys are supplied via
  `fly secrets set` as real env vars, read directly by the node (no config-file substitution step
  since there is no config file) — never baked into the image, never logged. DSNs go through
  `lb-secrets`, not the image. SQLite needs no secret.

## Example flow

1. **Local proof (30 s loop).** `make fly-local` builds the `deploy/common/Dockerfile`
   image and runs it behind mkcert TLS on `https://localhost`. Log in as the seeded dev
   user, open Datasources → `demo-buildings` probes green, open Data Studio → query it.
   Iterate; `make fly-local-down` when green.
2. **First-time Fly setup.** `fly apps create <app>`; `fly volumes create lb_data
   --region <r> --size 1`; `fly secrets set LB_SIGNING_KEY=… [model keys…]`.
3. **Deploy.** `make fly-deploy` → `fly deploy --config deploy/fly/fly.toml --dockerfile
   deploy/common/Dockerfile --ignorefile .dockerignore --remote-only .`.
   The remote builder: Rust release (`node` + `federation`) → pnpm SPA
   (`VITE_GATEWAY_URL=""`) → runtime image.
4. **Boot.** entrypoint sets the node's env vars directly (no config file — the node is
   env-driven), ensures `/data/{store,demo}`, starts Caddy, then starts the node in the
   background and `wait`s on it. The node mounts the gateway on `127.0.0.1:8731`,
   supervises the federation sidecar (`LB_FEDERATION_ENDPOINTS=127.0.0.1:0`), runs the
   boot seeders.
5. **Seed the demo source (idempotent, in the entrypoint).** A background task in the
   entrypoint polls `GET /workspaces` until the gateway answers, then — only if
   `/data/demo/buildings.db` doesn't already exist — runs `seed-demo-sqlite.sh` to
   register `demo-buildings` via the normal `datasource.add` verb. Best-effort: a seed
   failure never blocks the node from serving (rerun with `fly ssh console`).
6. **Verify.** `curl -sI https://<app>.fly.dev/` (SPA), `curl https://<app>.fly.dev/…`
   (a gateway route), open the app, log in, query the demo source.
7. **CI parity.** Every PR runs the `deploy-image` job — `docker build -f
   deploy/common/Dockerfile .` — so a change that breaks the image is caught before a
   deploy is ever attempted.

## Testing plan

Per `scope/testing/testing-scope.md` — but note this scope ships **no product code**, so
the "mandatory capability-deny + workspace-isolation" unit tests attach to the *code
paths this exercises*, which already have them (federation `net:*` deny, ws-scoped
`datasource.*`). What this scope must prove is that the **assets build and boot a real,
working node**:

- **Image builds (CI gate).** The new `deploy-image` CI job runs `docker build -f
  deploy/common/Dockerfile .` on every PR — green is the gate. Proves the shared
  Dockerfile stays valid and the context (via the shared ignore) is sane.
- **Boot smoke (real node — no mock, rule 9).** `deploy/fly/smoke.sh` (via `make
  fly-smoke`, default target the local plain-HTTP compose on `:8080`; point it at a live
  app with `SMOKE_URL=https://<app>.fly.dev`): `GET /` returns the SPA (`200`);
  `POST /login` returns a token; `GET /workspaces` with it includes the seed workspace.
- **Datasource seeded (real store + real sidecar).** The same smoke asserts a
  `POST /mcp/call` of `datasource.list` includes `demo-buildings` — proving the entrypoint
  seed landed against the real store + sidecar. (Probing `datasource.test` green and a
  `federation.query` returning rows are the deeper checks a follow-up can add; v1 asserts
  registration.) Reuses `seed-demo-sqlite.sh`; no fabricated data (rule 9).
- **Same-origin SPA↔gateway.** That `datasource.list` call is itself the proxy proof: a
  `POST /mcp/call` behind Caddy reaches the gateway (not the SPA fallback) — the dev-pulse
  "try_files ate the API" regression. If it hit the static handler it would return
  `index.html`, and the `jq` assertion would fail.
- **Persistence (manual).** Restart the container; the seeded workspace + datasource
  survive (proves the `/data` volume wiring, not an ephemeral store). Note: durable
  *sessions* across restart additionally require a stable `LB_SIGNING_KEY` (see the
  signing-key risk); smoke.sh doesn't assert this, so verify it by hand on first deploy.

Debug entries land under `debugging/deploy/` on first bug (Caddy ordering, context bloat,
volume-mount misses are the known dev-pulse failure modes to expect).

## Risks & hard problems

- **Caddy directive ordering** (dev-pulse pitfall #4): mixing `try_files` + `reverse_proxy`
  as bare directives lets the SPA fallback eat API routes → the frontend shows
  `Unexpected token '<'`. Mitigation: explicit `handle` blocks, first-match-wins, copied
  from the proven dev-pulse Caddyfile.
- **Build-context bloat** (pitfall #5): without the shared `.dockerignore` the context is
  GBs (`target/`, `node_modules/`, `doc-site/`) and the remote builder times out.
  Mitigation: one whitelist ignore in `deploy/common/`, referenced by every driver — the
  *reuse* requirement is also the fix.
- **`VITE_GATEWAY_URL` baked wrong.** It is baked at SPA build time; the deploy needs it
  **empty** (same-origin). A stray absolute value → CORS/mixed-content in the browser.
  Mitigation: set it explicitly to `""` in the Dockerfile's SPA stage; assert in smoke.
- **Signing-key handling.** The signing key must be a Fly secret (`LB_SIGNING_KEY`), never
  baked into the image. A wrong-shaped value (not 64 hex chars) doesn't crash-loop — `main.rs`'s
  `gateway_signing_key()` logs a warning and falls back to a fresh per-boot key, which just
  silently loses the "stable" property rather than failing boot. Mitigation: `smoke.sh` doesn't
  currently assert persistence across a real key; documented in `deploy/common/README.md`'s env
  table so an operator sets it deliberately.
- **Volume first-boot ownership/paths.** `/data/store` and `/data/demo` must exist and be
  writable before the node opens the store. Mitigation: entrypoint `mkdir -p` + a restart
  note (the dev-pulse `/data/pgdata` chown-race analog).
- **wasm guest at boot.** The node reads the `hello` wasm (and its `extension.toml`)
  unconditionally at startup from a path derived from the compile-time
  `CARGO_MANIFEST_DIR`. The Dockerfile builds the `hello` guest and copies both artifacts
  to the *exact* builder path (`/src/rust/extensions/hello/...`) so the baked-in path
  resolves at run time — no `HELLO_WASM` override needed. Easy to miss because dev resolves
  it from the cargo target dir; relocating the binary without those files would panic on
  boot.
- **Keeping the three drivers honest.** The whole value is "same image everywhere." If a
  driver quietly forks the Dockerfile, the guarantee is gone. Mitigation: the drivers may
  only *reference* `deploy/common/`, never copy it; the self-check and a doc note enforce
  this, and CI builds the common Dockerfile so drift is visible.

## Open questions — resolved (v1 shipped)

- **Signing-key persistence.** Resolved: `lb_auth::SigningKey` already had `from_seed(&[u8; 32])`
  (built for exactly this — "loaded from the keychain/secret store later"). Added one small,
  justified seam at the binary boundary: `gateway_signing_key()` in `rust/node/src/main.rs` reads
  `LB_SIGNING_KEY` (64 hex chars) and calls `from_seed`, falling back to `generate()` when unset or
  malformed — so `make dev`/tests are unchanged and a deploy sets one Fly secret for durable
  sessions. No core crate touched; this is the same thin role-aware layer §3.1 already permits.
- **Seed-in-entrypoint vs post-deploy.** Resolved: entrypoint. `deploy/common/entrypoint.sh` starts
  Caddy + the node, then in the background polls `GET /workspaces` until the gateway answers and
  runs `seed-demo-sqlite.sh` only if `/data/demo/buildings.db` doesn't exist yet — self-contained
  first boot, a no-op after, and best-effort (a seed failure never blocks the node from serving).
- **CI: build-only or push?** Shipped as build-only — the `deploy-image` job in
  `.github/workflows/ci.yml` runs `docker build -f deploy/common/Dockerfile .` on every PR/push to
  `master`. No push, no `fly deploy` from CI. Revisit once there's a staging app to point a release
  workflow at.
- **Region + VM size defaults.** Shipped `cdg` / `shared-cpu-1x` in `deploy/fly/fly.toml` (matches
  the dev-pulse precedent). Change before the first real `fly apps create` if a different region is
  wanted — it's one field.
- **App name + custom domain.** Shipped app name `lazybones` in `fly.toml`; no custom domain wired.
  `fly certs add` is a follow-up once a domain is chosen.
- **No unauthenticated `/health` route (discovered during implementation).** The gateway has no
  health endpoint, so `fly.toml`'s `[[http_service.checks]]` probes `GET /` (Caddy always answers,
  independent of node health) rather than a deeper gateway check. `deploy/fly/smoke.sh` is the real
  end-to-end proof (login, `/workspaces`, `datasource.list`). A follow-up could add a cheap
  unauthenticated `/health` gateway route for a tighter Fly check, but that's a core-crate change
  out of this scope's "no product code" boundary.

## Related

- README `§3` (the non-negotiable rules — 1 symmetric nodes, 2 one datastore, 5/6
  caps+workspace, 10 core-knows-no-extension), `§5` (deployment personas).
- `datasources/sqlite-datasource-demo-scope.md` — the sqlite kind + `seed.py --sqlite` +
  `make seed-demo-sqlite` this deploy leans on; `datasources/datasources-scope.md` (the
  federation sidecar).
- `desktop/desktop-standalone-backend-scope.md` + `desktop-federation-bundle-scope.md` —
  the desktop `full` build is the *other* "standalone node with the federation sidecar
  bundled" target; this scope is its server-side twin (same wiring, different shell).
- `sync/` — multi-node/HA, deliberately out of scope here.
- Reference runbook: `/home/user/code/rust/dev-pulse/FLY.md` (the pattern this adapts;
  its bundled-PG and OAuth steps are dropped).
- Assets: `deploy/common/` (shared build inputs), `deploy/fly/` (the Fly driver),
  `docker/postgres/seed-demo-sqlite.sh` (the demo seed), `Makefile` (`fly-*` targets).
- Skill: **N/A for a new drivable MCP surface** — this adds none. The operator runbook is
  a `docs/public/deploy/deploy.md` page (promoted on ship) + the `make fly-*` targets, not
  an agent-driven tool. (If we later expose deploy as an `lb` CLI verb, it gets a skill then.)
