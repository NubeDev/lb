# deploy/common/ — shared build inputs (one source of truth)

The build inputs that **every deploy driver reuses verbatim**: local Docker, GitHub CI,
and the Fly.io deploy all point at these files. One Dockerfile, one Caddyfile, one
entrypoint, one `.dockerignore`, one local compose file. A driver may only *reference* these
— never fork them — so the image you run locally is the image CI builds is the image Fly
runs (README §3 rule 1: one binary, config selects the role).

Scope: [`../../docs/scope/deploy/fly-deploy-scope.md`](../../docs/scope/deploy/fly-deploy-scope.md).

## Contents

| File | Purpose |
|---|---|
| `Dockerfile` | Three stages: Rust release build of `node` + the `federation` sidecar (postgres feature) + the `hello` wasm guest → pnpm SPA build (`VITE_GATEWAY_URL=""`, same-origin) → debian-slim runtime with Caddy + tini + the binaries + the SPA `dist/`. Build context is the **repo root**. |
| `Caddyfile` | SPA-explicit + allow-all reverse proxy (`handle` blocks, first-match-wins; `auto_https off` — fly-proxy terminates TLS). Serves `/`, `/assets/*`, `/shims/*`, static roots; proxies everything else to `127.0.0.1:8731`. |
| `entrypoint.sh` | Ensure `/data/{store,demo}` → start Caddy → start the node → best-effort seed `demo-buildings.db` once the gateway answers → wait on the node. The node is entirely env-driven (no config file — see `rust/node/src/main.rs`/`federation.rs`), so there is no template to render. |
| `.dockerignore` | The whitelist ignore content, referenced by every driver so the build context is small and identical everywhere. |
| `compose.yml` | Local plain-HTTP compose stack over the same image (a named volume for `/data`). |

## Runtime env the node reads (set by a driver, not baked into the image)

| Var | Default (set in `entrypoint.sh`) | Purpose |
|---|---|---|
| `LB_SIGNING_KEY` | unset → a fresh key per boot | 64 hex chars (32-byte Ed25519 seed). **Set this in any long-lived deploy** — without it every session dies on restart (`rust/node/src/main.rs` `gateway_signing_key()`). Generate with `openssl rand -hex 32`. |
| `LB_WORKSPACE` | `acme` | The boot/seed workspace. |
| `LB_SEED_USER` | `user:ada` | The dev identity seeded as `workspace-admin` at boot. |
| `LB_STORE_PATH` | `/data/store/node-store` | The embedded SurrealKV directory — on the volume. |
| `LB_FEDERATION_ENDPOINTS` | `127.0.0.1:0` | The sqlite-only convention endpoint (no bundled Postgres — rule 2). |

## Non-negotiables

- **No hosted or bundled Postgres.** The node embeds SurrealDB (rule 2). External SQL is a
  federated source through the sidecar, never a second authority. The default deploy uses
  **SQLite** datasources (the `127.0.0.1:0` convention).
- **Secrets are never baked.** Signing/model keys come from the environment
  (`fly secrets set`) and are read directly by the node as env vars — there is no config
  file to substitute them into.
- **Same-origin SPA.** The SPA is built with `VITE_GATEWAY_URL=""` so the browser issues
  relative API calls that Caddy proxies to the gateway.
