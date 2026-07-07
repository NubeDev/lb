# desktop/docker/

The Docker build environment for the Linux `lazybones-shell` desktop binary. One image,
three BuildKit stages in one `Dockerfile` (the scope's "one image, many output types"
decision):

| Stage | What it is | Used by |
| --- | --- | --- |
| `base` | Ubuntu 22.04 + webkit2gtk-4.1 + Rust stable + Node 22 + pnpm 9 | **dev** — bind-mount the repo, `docker run` |
| `builder` | `base` + COPY repo + RUN build | reached transitively by `artifact` |
| `artifact` | `FROM scratch`, the binary only | **CI** — `--output type=tar` extraction |

The image is **build-only** — the shipped binary is a windowed desktop app, not a
container workload. Runtime node images live in the repo-root `docker/` workspace.

## Quick start

From the **repo root** (so `COPY . /build` and the repo-root `.dockerignore` apply), or
via the Makefile which handles paths:

```bash
make -C desktop linux-executable   # dev: binary lands at ui/src-tauri/target/release/lazybones-shell
make -C desktop artifact           # CI : lazybones-shell-linux-x86_64.tar
make -C desktop smoke              # boot the binary under xvfb (liveness check)
```

Or raw docker (equivalent to the Makefile targets):

```bash
cd <repo-root>

# 1. Build the base toolchain image (once, or after touching Dockerfile/build.sh).
docker build -f desktop/docker/Dockerfile --target base -t lazybones-shell-builder:local .

# 2a. Dev — bind-mount the repo; binary lands on the host.
docker run --rm --user "$(id -u):$(id -g)" \
  -v "$PWD":/build \
  -v lb-cargo:/usr/local/cargo/registry \
  -v lb-pnpm:/usr/local/pnpm/store \
  lazybones-shell-builder:local

# 2b. CI — multi-stage build, extract just the ELF as a tar.
docker build -f desktop/docker/Dockerfile --target artifact \
  --output type=tar,dest=lazybones-shell-linux-x86_64.tar .
```

## Files

- `Dockerfile` — the three-stage image definition. Build context is the **repo root**
  (so the builder stage's `COPY . /build` brings in `ui/`, `packages/`,
  `rust/crates/*` — the path-dep tree the shell links against).
- `build.sh` — the entrypoint. One verb: `pnpm install --frozen-lockfile` →
  `cd ui && pnpm tauri build --no-bundle -- --features desktop`. Used unchanged in both
  invocation modes (dev bind-mount, CI COPY).
- `README.md` — this file.

## Dev mode (bind-mount, host UID)

The container runs as your host UID/GID (`--user $(id -u):$(id -g)`), so build output
— `ui/src-tauri/target/`, `ui/node_modules/`, the workspace `node_modules/` — lands
**owner-correct on the host**. No `sudo chown` after every build (the rejected
alternative in the scope's Decisions). The cargo registry and pnpm store live in named
volumes (`lb-cargo`, `lb-pnpm`) so iteration is warm across runs; `make clean` clears
them.

The image pre-creates the cache roots and `/home/builder` world-writable (the OpenShift
arbitrary-UID convention) so any host UID can write them without a chown entrypoint hack.

The binary drops at `ui/src-tauri/target/release/lazybones-shell`. **Run it directly** —
no Docker needed to *run* the desktop app, only to build it.

## CI mode (artifact stage)

The `artifact` stage is `FROM scratch` with just the ELF. Extract with BuildKit's
`--output type=tar`:

```bash
docker build -f desktop/docker/Dockerfile --target artifact \
  --output type=tar,dest=lazybones-shell-linux-x86_64.tar .
```

The tar contains a single `lazybones-shell`. The CI job uploads it as the workflow
artifact the packaging scope names. No `docker create` + `docker cp` follow-up — one
command, reproducible from a clean checkout.

## In-container smoke

`xvfb` is installed (the scope's "same image runs the smoke" decision). `make smoke`
launches the just-built binary under `xvfb-run` and asserts the process stays up for 5s.
The full IPC round-trip smoke (`channel_post` → `channel_history` against the embedded
node, per the scope's testing plan) is wired in the building session — the image just
provides the xvfb runtime and the `--entrypoint bash` override seam.

## Why Ubuntu 22.04

It is the minimum LTS line that ships **webkit2gtk-4.1** (the Tauri v2 requirement —
4.0 is not enough). Building *on* 22.04 guarantees the binary's dynamic-link line
matches the floor the runtime contract tells end users to install. See the scope's
Decisions section.

## Toolchain versions (match CI exactly)

| Tool | Version | Source |
| --- | --- | --- |
| Ubuntu | 22.04 | base image |
| Rust | stable | rustup (matches `.github/workflows/ci.yml` `dtolnay/rust-toolchain@stable`) |
| Node | 22 | NodeSource (matches `actions/setup-node@v4` `node-version: 22`) |
| pnpm | 9 | `npm install -g pnpm@9` (matches `pnpm/action-setup@v4` `version: 9`) |

Bumping any of these is a scope-level decision — the runtime contract's minimum distro
line moves with the base image, and the CI matrix must move with them.

## Related

- Scope: [`../../docs/scope/desktop/desktop-build-container-scope.md`](../../docs/scope/desktop/desktop-build-container-scope.md)
- Parent scope: [`../../docs/scope/desktop/desktop-packaging-scope.md`](../../docs/scope/desktop/desktop-packaging-scope.md)
- Linux build docs: [`../docs/linux/README.md`](../docs/linux/README.md)
- Shell crate: [`../../ui/src-tauri/`](../../ui/src-tauri/) — the `desktop` feature seam
  (`Cargo.toml`), `tauri.conf.json`, `build.rs`, `src/desktop.rs`.
- Repo-root `.dockerignore` — keeps the `COPY . /build` context small.
