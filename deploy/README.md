# deploy/

How a Lazybones `node` is shipped to a target host. **One image, many drivers**: the build
inputs live once in [`common/`](common/) and are reused verbatim by every driver, so the
image you run locally is the image CI builds is the image the target runs (README §3
rule 1 — one binary, config selects the role).

- [`common/`](common/) — the shared build inputs (Dockerfile, Caddyfile, entrypoint,
  `.dockerignore`, local compose). The single source of truth.
- [`fly/`](fly/) — the **Fly.io** driver: a single Machine running a Caddy-fronted node
  with an embedded SurrealDB store on a volume and the federation sidecar in SQLite mode.
  No bundled Postgres.

A driver may only *reference* `common/`, never fork it. Adding a new target (a VPS, a
different PaaS) means a new thin driver dir here, pointing at the same `common/` inputs.

Scope: [`../docs/scope/deploy/fly-deploy-scope.md`](../docs/scope/deploy/fly-deploy-scope.md).
