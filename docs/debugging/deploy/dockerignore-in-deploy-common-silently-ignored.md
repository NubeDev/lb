# `deploy/common/.dockerignore` was silently ignored — build context balloons / COPY fails

## Symptom

The Fly deploy scope planned a `deploy/common/.dockerignore` — one whitelist ignore file,
"shared verbatim" by local Docker, CI, and Fly. Implementing it, `docker build -f
deploy/common/Dockerfile .` (repo root context) failed on
`COPY docker/postgres/seed.py ...` with `file not found in build context or excluded by
.dockerignore`, even though `deploy/common/.dockerignore` explicitly whitelisted that path.

## Root cause

Docker resolves `.dockerignore` from the **build context root**, never from next to the
Dockerfile — regardless of `-f <path>/Dockerfile`. A per-Dockerfile ignore file only works
via `buildx build --ignorefile`, a **buildx-only** flag. Plain `docker build` and `docker
compose build` (what local dev and the classic CI runner use) always read the context
root's `.dockerignore` and have no way to point elsewhere. The repo already had a root
`.dockerignore` (denylist-style, for `desktop/docker/Dockerfile`) that excluded
`docker/postgres/` wholesale — that's the file that was actually governing the build, and
it excluded a path the deploy image's `COPY` needed.

The planned `deploy/common/.dockerignore` was real, correctly-scoped, and completely inert
for two of the three drivers (local Docker, CI). Only Fly's remote builder (which does use
buildx, so `--ignorefile deploy/common/.dockerignore` works) would ever have honored it —
and even that would have then diverged from what `docker build`/`docker compose build`
actually strip, defeating the "same context everywhere" goal.

## Fix

Deleted `deploy/common/.dockerignore`. The **repo-root** `.dockerignore` is now the one
ignore file every repo-root-context build shares (desktop's included): kept its existing
denylist shape and added a narrow re-include carve-out for the seed script the deploy
image's `COPY docker/postgres/seed.py ...` needs:

```
docker/postgres/
!docker/postgres/seed.py
!docker/postgres/generators.py
!docker/postgres/inventory.py
!docker/postgres/tags.py
!docker/postgres/sinks_sqlite.py
!docker/postgres/seed-demo-sqlite.sh
```

`deploy/fly/fly.toml`'s `make fly-deploy` now passes `--ignorefile .dockerignore` (still
explicit, for the Fly buildx path, but pointing at the SAME file local/CI implicitly use).

## Lesson

"One ignore file, referenced by every driver" has to mean the same *file*, not
same-content copies — Docker's ignore-file resolution rule (context root, always, unless
you're specifically on the buildx `--ignorefile` path) makes a copy silently dead weight
for whichever drivers don't pass that flag. Verify by actually running the non-buildx
build locally before trusting a "shared" ignore file plan.

## Regression coverage

`.github/workflows/ci.yml`'s `deploy-image` job runs the exact same
`docker build -f deploy/common/Dockerfile .` (classic builder, repo-root `.dockerignore`)
on every PR — a re-introduced per-Dockerfile ignore file would immediately break that job
the same way it broke the local build here.

Session: `docs/sessions/deploy/fly-deploy-implementation-session.md`.
