# Deploy image: node starts, logs nothing, exits 1 with "No such file or directory"

## Symptom

Running the built deploy image (`docker run lazybones:local-test`), Caddy started fine but
the node process printed **no** boot output at all (not even the very first `boot seed: …`
line) and the container exited with code 1 and the bare message
`Error: No such file or directory (os error 2)`.

## Root cause

`rust/node/src/main.rs` reads the `hello` extension's manifest via a path baked in at
**compile time**: `env!("CARGO_MANIFEST_DIR")/../extensions/hello/extension.toml`.
`CARGO_MANIFEST_DIR` is the absolute path the Rust builder stage used —
`/src/rust/node` — so the runtime image needs that path to resolve, which the Dockerfile
already arranged by copying `extension.toml` + the built wasm to
`/src/rust/extensions/hello/...`.

But `openat` resolves a path **component by component** — including the `..` hop — and
`/src/rust/node` itself was never created in the runtime stage (only its sibling
`/src/rust/extensions/hello/` was copied). A missing *intermediate* path component makes
the whole `openat("/src/rust/node/../extensions/hello/extension.toml")` fail `ENOENT`,
even though the directory it logically resolves to (`/src/rust/extensions/hello/`) exists.
`strace -f -e trace=openat` on the failing binary was what surfaced this — the error
message alone (bare `os error 2`, no path) gave no clue which file was missing.

## Fix

`RUN mkdir -p /src/rust/node` in the runtime stage before the `COPY --from=rust-builder
.../extension.toml` step (`deploy/common/Dockerfile`). The directory only needs to exist
for `..` traversal — nothing is ever read from inside it.

## Lesson

A Rust binary's `env!("CARGO_MANIFEST_DIR")`-derived paths are baked in as **absolute
builder-stage paths**. Reusing that constant path across a multi-stage Dockerfile requires
mirroring the *builder's full directory chain* in the runtime stage, not just the leaf
directory the file logically lives in — any `..` in the compiled-in path needs its
intermediate directory to physically exist, even empty.

## Regression coverage

`.github/workflows/ci.yml`'s `deploy-image` job proves the image still *builds*, but not
that it *boots* — build-only per the scope's "CI: build-only or push?" decision. The real
regression guard is `deploy/fly/smoke.sh` run manually (or via `make fly-smoke`) against a
freshly built image before every deploy; a reintroduced missing-`mkdir` would fail loudly
at `docker run` with the exact "no boot output, exit 1" signature above.

Session: `docs/sessions/deploy/fly-deploy-implementation-session.md`.
