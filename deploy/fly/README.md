# deploy/fly/ — the Fly.io driver

A thin driver over [`../common/`](../common/): it *references* the shared Dockerfile,
Caddyfile, entrypoint, and `.dockerignore`, and adds only the
Fly-specific glue. One Fly Machine runs a Caddy-fronted Lazybones `node` with an embedded
SurrealDB store on a persistent volume and the federation sidecar in SQLite mode. **No
bundled Postgres** — the node embeds its one datastore (README §3 rule 2).

Scope: [`../../docs/scope/deploy/fly-deploy-scope.md`](../../docs/scope/deploy/fly-deploy-scope.md).
Pattern reference: `/home/user/code/rust/dev-pulse/FLY.md` (adapted — its bundled-PG and
OAuth steps are dropped).

## Contents

| File | Purpose |
|---|---|
| `fly.toml` | Fly app config — region `cdg`, `shared-cpu-1x`, `[build]` → `deploy/common/Dockerfile`, `[http_service]` `:8080`, `[mounts]` `/data` → volume `lb_data`, non-secret env. No `release_command` (seeding runs in the entrypoint; release VMs get no volume). |
| `compose.fly-local.yml` + `Caddyfile.local` | Local **HTTPS pre-flight** — the same image behind mkcert TLS on `https://localhost`. Prove a deploy in ~30 s before the ~5–10 min Fly round-trip. |
| `smoke.sh` | Boot smoke against a running container: `POST /login` → token, `GET /workspaces` includes the ws, `GET /` returns the SPA, `demo-buildings` registered (`datasource.list`) — proving Caddy proxies the gateway hop, not the SPA fallback. |

## Commands (Makefile `fly-*` targets)

```
make fly-local        # build deploy/common/Dockerfile, run behind mkcert TLS on https://localhost
make fly-local-down   # tear down (keeps volumes)
make fly-smoke        # run smoke.sh (default: local plain-HTTP compose on :8080)
make fly-deploy       # fly deploy --config deploy/fly/fly.toml \
                       #   --dockerfile deploy/common/Dockerfile \
                       #   --ignorefile deploy/common/.dockerignore --remote-only .
make fly-logs / fly-ssh / fly-status
```

Local HTTPS pre-flight needs an mkcert cert pair under `deploy/fly/certs/` (gitignored):

```
mkcert -install                                            # one-time, installs a local CA
mkdir -p deploy/fly/certs && cd deploy/fly/certs
mkcert localhost 127.0.0.1
mv localhost+1.pem localhost.pem
mv localhost+1-key.pem localhost-key.pem
```

First-time setup (once per app):

```
fly apps create <app>
fly volumes create lb_data --region <r> --size 1 -a <app> --yes
fly secrets set -a <app> LB_SIGNING_KEY=…   # + any model/API keys; never bake into the image
```
