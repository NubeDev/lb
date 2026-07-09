# Session: Fly.io deploy implementation

## The ask

Build out `docs/scope/deploy/fly-deploy-scope.md` (written in a prior session): a
one-command Fly.io deploy of a Lazybones `node` — Caddy-fronted, embedded SurrealDB on a
persistent volume, federation sidecar in SQLite mode, no bundled Postgres — with the build
inputs shared verbatim across local Docker, GitHub CI, and Fly.

## What shipped

- `deploy/common/Dockerfile` — three stages: Rust release build of `node` + `federation`
  (postgres feature) + the `hello` wasm guest → pnpm SPA build (`VITE_GATEWAY_URL=""`) →
  debian-slim runtime (Caddy + tini). Build context is the repo root (no sibling-dir
  path-dep the way dev-pulse needed `../starter`).
- `deploy/common/Caddyfile` — SPA-explicit `handle` blocks + allow-all reverse proxy to
  `127.0.0.1:8731`.
- `deploy/common/entrypoint.sh` — start Caddy + the node, then a background poll of
  `GET /workspaces` that seeds the demo SQLite datasource once the gateway answers
  (idempotent, best-effort). No config-template step — see below.
- `deploy/common/compose.yml` — local plain-HTTP driver.
- `deploy/fly/fly.toml`, `compose.fly-local.yml` + `Caddyfile.local` (mkcert HTTPS
  pre-flight), `smoke.sh` (real HTTP checks: login, `/workspaces`, `datasource.list`).
- `Makefile` — `fly-local`/`fly-local-down`/`fly-smoke`/`fly-deploy`/`fly-logs`/`fly-ssh`/
  `fly-status`.
- `.github/workflows/ci.yml` — `deploy-image` job, build-only, every PR.
- One small core-adjacent change: `rust/node/src/main.rs` gained `gateway_signing_key()`,
  reading `LB_SIGNING_KEY` (64 hex chars) via the existing `SigningKey::from_seed` seam,
  falling back to `generate()` — so a deploy's sessions survive a restart. `make dev`/tests
  unchanged (no env set → same behavior as before).
- `docs/debugging/deploy/` — two entries for real bugs hit and fixed (below).

## Discovered during implementation (scope doc corrected in place)

The written scope assumed a dev-pulse-style `config.toml.tmpl` + `envsubst` step. Reality:
`rust/node/src/main.rs`/`federation.rs` are **entirely env-driven** — there is no config
file. `entrypoint.sh` ended up simpler than planned: just export env vars, no template to
render. Updated `fly-deploy-scope.md`'s "Intent/approach" and "Open questions" sections in
place to match what shipped (a scope doc is a live spec, not a changelog).

## Two real bugs, both logged + fixed

1. **`deploy/common/.dockerignore` was silently inert.** Docker resolves `.dockerignore`
   from the build CONTEXT ROOT always, not next to the Dockerfile — a per-Dockerfile
   ignore file needs `buildx build --ignorefile`, which plain `docker build`/`docker
   compose build` never pass. The repo-root `.dockerignore` (desktop-scoped) was the file
   actually governing the build and excluded `docker/postgres/` wholesale, breaking the
   deploy image's `COPY docker/postgres/seed.py`. Fixed by deleting the planned
   `deploy/common/.dockerignore` and adding a narrow re-include carve-out to the
   repo-root file instead — genuinely one shared file now.
   → `docs/debugging/deploy/dockerignore-in-deploy-common-silently-ignored.md`
2. **Node exited 1 with no boot output, bare `os error 2`.** `main.rs` reads the `hello`
   manifest via a `CARGO_MANIFEST_DIR`-baked absolute path (`/src/rust/node/../extensions/
   hello/extension.toml`); the runtime stage never created the now-empty `/src/rust/node`
   directory the `..` hop traverses, so `openat` ENOENT'd on the missing intermediate
   component. Found via `strace -f -e trace=openat` inside the built image. Fixed with one
   `RUN mkdir -p /src/rust/node`.
   → `docs/debugging/deploy/node-boots-then-exits-hello-manifest-enoent.md`

## Testing (real, no mocks — rule 9)

All done against the actual built image, not a simulation:

- `docker build -f deploy/common/Dockerfile .` — green (this environment has no `buildx`,
  so the BuildKit `--mount=type=cache` lines were dropped from the Dockerfile to keep it
  portable to classic `docker build`; CI/Fly still work fine without them, just without the
  registry/target cache speedup).
- `docker run` the built image (no volume) → boot log shows the real seed pipeline: role
  seeding, 9 agent defs, 9 personas, `hello.echo -> {"echo":"hi"}`, federation sidecar
  installed, demo dataset generated (`seed.py --sqlite`, real building/meter/point rows)
  and registered as `demo-buildings` via the live `datasource.add` verb.
- `deploy/fly/smoke.sh` against the running container: `GET /` → 200, `POST /login` →
  token, `GET /workspaces` includes `acme`, `datasource.list` includes `demo-buildings` —
  all green.
- **Persistence proof:** ran again with a named volume + `LB_SIGNING_KEY` set, minted a
  token, `docker restart`, reused the SAME pre-restart token against `/workspaces` → still
  200. Proves the signing-key seam actually delivers stable sessions, not just "login still
  works" (which a fresh per-boot key would also pass).
- `docker restart` with no signing key set (throwaway smoke) → clean reboot, no
  crash-loop, matching the documented "malformed/missing → fresh key" fallback.

Not run (needs real infra this session didn't have): `fly deploy` itself, the mkcert HTTPS
pre-flight (`make fly-local`), and a real `fly apps create`/`fly volumes create` first-time
setup. `deploy/fly/README.md` documents those steps for whoever runs them next.

## Open questions resolved

See `docs/scope/deploy/fly-deploy-scope.md`'s "Open questions — resolved (v1 shipped)"
section — signing-key persistence, seed timing, CI scope, region/app defaults, and the
discovered "no `/health` route" gap are all recorded there with what shipped and why.
