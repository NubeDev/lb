# Desktop build container scope — a Docker image that builds the Linux shell binary

Status: scope (the ask). Sibling to `desktop-packaging-scope.md` — it makes that scope's
"runs on a real dev box or CI" line **reproducible**. Promotes to
`public/desktop/build-container.md` once shipped.

The packaging scope (`desktop-packaging-scope.md`) runs
`tauri build --no-bundle -- --features desktop` on a machine with the webkit2gtk-4.1
toolchain — which the sandboxed dev box deliberately lacks (it links via `zig`, has no
`cc`, and cannot install the apt set; see `rust/.cargo/config.toml`). This scope ships
that machine **as a Docker image**: one Debian/Ubuntu-based container with
webkit2gtk-4.1 + Rust + pnpm that produces the bare ELF (and, later, deb/rpm/AppImage)
from a clean checkout. Reproducible, host-pollution-free, one image dev and CI both
use. The image is **build-only** — the shipped binary is a windowed desktop app, not a
container workload (the runtime node images live in the repo-root `docker/` workspace).

## Goals

- A `Dockerfile` (under `desktop/docker/`) that builds `lazybones-shell` for
  `x86_64-unknown-linux-gnu` from a clean checkout — no host toolchain assumed beyond
  Docker itself.
- **Reproducible:** pinned base image (digest), pinned Rust (stable via rustup),
  Node 22 + pnpm 9 — matching `.github/workflows/ci.yml` exactly so "it built locally"
  and "it built in CI" stop diverging.
- **Binary extraction:** the ELF lands on the host (dev) or as a CI artifact — never
  trapped in a dead container. One named seam for both modes.
- **Fast iteration:** cargo + pnpm caches survive across `docker run`s (BuildKit
  `--mount=type=cache` or named volumes); a no-op rebuild is seconds, not minutes.
- **One image, many output types:** the same image builds the bare-executable slice now
  and deb/rpm/AppImage later — the container is the *build environment*, the output
  *type* is the command you hand it (the `tauri build --bundles <type>` flag, not a
  new image).

## Non-goals

- **darwin or windows targets in the container.** macOS needs Apple's toolchain
  (legally not containerizable); Windows needs MSVC on a Windows host (cross-compiling
  from Linux is already rejected in the packaging scope — the shell embeds the heavy
  native tree). Docker here = **Linux-x86-64 only**. The platform-targets axis
  (`platform-targets/platform-targets-scope.md`) records the rest.
- **A runtime image.** The shipped binary does NOT run in a container — it opens a
  window. Repo-root `docker/` owns runtime node images; this builder is separate and
  build-only.
- **Baking the repo into the dev image.** Dev bind-mounts the repo; CI COPYs it. Same
  image, two invocation modes (below).
- **Multi-arch container builds** (arm64 native) — follow-up once `linux-arm64` is on
  the platform-targets axis.
- **Distributing the image on a registry** — local `docker build` + CI use only for
  this slice; pushing to GHCR/quay with signing is its own follow-up.
- **snap.** Explicitly rejected as a Linux output type — it requires `snapd` on the
  host (a runtime dep the bare-binary contract deliberately avoids), drags in Canonical
  store / auto-update coupling, and is widely disliked on Linux desktops. **Ubuntu is
  supported first-class via the bare executable (run directly) and the future `.deb`**
  (Ubuntu is deb-based); snap adds nothing those two don't already cover. AppImage
  remains the self-contained option if a no-runtime-deps format is later wanted.

## Intent / approach

The shell crate is its own cargo workspace (`ui/src-tauri/Cargo.toml`) that pulls
`lb-host`/`lb-auth`/`lb-inbox` via path deps from `rust/crates/*`, and the React build
pulls `@nube/*` from `packages/*` via the repo-root `pnpm-workspace.yaml`. **The image
must therefore see the whole repo**, not just `ui/`. That shapes both invocation modes.

**Layers (cheapest-to-rebuild first):**

1. **Base + apt** — `ubuntu:22.04` by digest (the minimum LTS line that ships
   webkit2gtk-4.1; matches the packaging scope's runtime contract). Install exactly
   the packaging scope's apt set: `build-essential`, `pkg-config`,
   `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libsoup-3.0-dev`,
   `libjavascriptcoregtk-4.1-dev`, `librsvg2-dev`, plus `ca-certificates`, `curl`,
   `git`, `file`, `xvfb` (for the in-container smoke).
2. **Rust** — rustup + `stable` toolchain, host target `x86_64-unknown-linux-gnu`.
3. **Node 22 + pnpm 9** — match `.github/workflows/ci.yml` exactly.
4. **WORKDIR /build** + the entrypoint script.

**Two invocation modes, one image:**

- **Dev (bind-mount, host UID):** repo and target dir on the host; cargo + pnpm caches
  in named volumes; the container runs as the host UID/GID so bind-mounted files come
  out owner-correct. Output lands at the host path
  `ui/src-tauri/target/release/lazybones-shell` naturally — no extraction step.
  `docker run --rm --user "$(id -u):$(id -g)" -v "$PWD":/build -v lb-cargo:/usr/local/cargo/registry -v lb-pnpm:/root/.local/share/pnpm/store lazybones-shell-builder`
  (the image pre-creates HOME + the cache dirs writable by arbitrary UIDs so `--user`
  Just Works).
- **CI (COPY + artifact stage):** a multi-stage `Dockerfile` that COPYs the repo,
  builds, and a final `FROM scratch` stage COPYs only the binary out. Extracted with
  `docker build --output type=tar,dest=-` (piped to the artifact file) — one command,
  no follow-up `cp`. No bind-mount, fully reproducible from a clean checkout.

**Entrypoint** (one script, `desktop/docker/build.sh`):
`pnpm install --frozen-lockfile` at repo root → `cd ui && pnpm tauri build --no-bundle -- --features desktop`.
The `beforeBuildCommand: "pnpm build"` in `tauri.conf.json` runs the Vite build into
`ui/dist/`; cargo then compiles the shell with the `desktop` feature on; the ELF drops
at `ui/src-tauri/target/release/lazybones-shell`.

**The zig-linker hack does NOT leak.** `rust/.cargo/config.toml`'s zig `cc` shim is
applied by cargo only to manifests at-or-under `rust/`; `ui/src-tauri` is a sibling
workspace, so its build never consults that config. The container uses the real `cc`
from `build-essential` — no zig, no leak. Worth a regression assertion (below) so a
future file move doesn't silently break this.

**Alternatives rejected:**

- *Tauri's published reference Dockerfiles as-is.* They build a generic sample; they
  don't model this repo's path-deps + pnpm-workspace shape. Used as a starting point,
  not copied.
- *Pre-baking the repo into one fat dev image.* Rebuilds the image on every code change
  — the slow path. Bind-mount wins for dev; the CI stage handles reproducibility.
- *Running the build on the GitHub runner directly (apt-install webkit in the workflow
  file).* Works, but every CI run re-downloads ~500 MB of apt packages and there is no
  local-dev parity. The image pays the toolchain cost once.

## How it fits the core

- **Symmetric nodes (rule 1):** the image produces ONE binary persona (`workstation`
  = node + window). No role branch in core crates; the image is pure toolchain. The
  `desktop` cargo feature is still the only switch, exactly as in the packaging scope.
- **Placement:** build-time only. The image runs nowhere as a node — it emits a binary
  that boots the same in-process node every other persona boots.
- **No mocks (rule 9):** real toolchain, real cargo, real webkit dynamic link, real
  SurrealDB/Zenoh compiled in. The in-container xvfb smoke boots the actual binary
  against the actual embedded node — the same real-store/real-bus proof the packaging
  scope names.
- **One responsibility per file (rule 8):** `Dockerfile` (the image), `build.sh` (the
  entrypoint), `README.md` (the docs) — three files in `desktop/docker/`, not one
  catch-all.
- **SDK/WIT impact:** none.

## Example flow

1. **One-time:** `docker build -t lazybones-shell-builder desktop/docker`.
2. **Dev:** from repo root,
   `docker run --rm -v "$PWD":/build -v lb-cargo:/usr/local/cargo/registry -v lb-pnpm:/root/.local/share/pnpm/store lazybones-shell-builder`.
3. The container runs `pnpm install --frozen-lockfile`, then
   `cd ui && pnpm tauri build --no-bundle -- --features desktop`.
4. The binary appears at `ui/src-tauri/target/release/lazybones-shell` on the host
   (bind-mount) — `./lazybones-shell` opens the window; no Docker needed to *run* it.
5. **CI:** a `desktop-build.yml` job `docker build -f desktop/docker/Dockerfile.ci .`
   (the multi-stage variant), extracts the binary from the scratch stage, and uploads
   it as the workflow artifact (the same artifact the packaging scope's CI lane names
   `lazybones-shell-linux-x86_64`).
6. **Smoke (CI, same image):** `docker run ... lazybones-shell-builder xvfb-run -a
   ./ui/src-tauri/target/release/lazybones-shell` + the IPC liveness round-trip.

## Testing plan

Per `scope/testing/testing-scope.md` (rule 9 — real, no mocks):

- **Image-build gate:** `docker build desktop/docker` succeeds from a clean checkout;
  failure blocks.
- **Binary gate:** the container produces a runnable ELF; `file lazybones-shell` shows
  `ELF 64-bit LSB shared object, x86-64, dynamically linked`.
- **Smoke (in-container):** `xvfb-run` the produced binary, assert the process stays up
  and one real IPC round-trip (`channel_post` → `channel_history`) against the embedded
  node succeeds. This is the packaging scope's smoke, now host-toolchain-free — the
  whole proof runs inside the image.
- **Reproroducibility:** build the image twice on different hosts; diff the produced
  binary hashes. Document the non-reproducibility sources (build-id / timestamps) and
  either strip them or accept the delta with a note.
- **Regression — no zig leak:** `docker run --rm lazybones-shell-builder sh -c
   'cd ui/src-tauri && cargo build -p lazybones-shell --features desktop -v 2>&1 |
   grep -c zigcc'` returns 0. Proves the `rust/.cargo/config.toml` hack does not leak
   into the container build.
- **Headless path regression:** `cargo build -p lazybones-shell` (no feature) inside
  the container still succeeds — the feature seam the packaging scope relies on stays
  intact.

## Risks & hard problems

- **File ownership on bind-mount.** cargo/pnpm as root in the container writes
  root-owned files into the host's `ui/src-tauri/target/` and `node_modules/`. Either
  run the entrypoint as the host UID/GID (`--user $(id -u):$(id -g)` + a matching
  HOME), or accept a `sudo chown -R` step. The dev-iteration UX hinges on getting this
  right — say which way in the skill doc.
- **Image size.** webkit dev headers + Rust toolchain + Node = multi-GB. Acceptable
  for a builder; the `FROM scratch` artifact stage keeps the *output* tiny. State the
  size in the skill doc so nobody is surprised.
- **Cache thrash without mounts.** pnpm store + cargo registry are large; every cold
  rebuild is minutes. BuildKit `--mount=type=cache,target=...` is the clean answer in
  CI; named volumes for dev. If omitted, the image is "correct but slow" — a known
  trap.
- **webkit2gtk soname drift.** Pinning the base image by digest fixes the version
  forever; bumping the base shifts the soname and the runtime contract's minimum
  distro line with it. The `Depends` line in a future deb must read the soname the
  image built against, not "whatever 4.1 is current".
- **Path-dep fan-out.** A `cargo clean` anywhere under `rust/crates/` invalidates a
  huge swath of the shell build (lb-host pulls the whole spine). The cache mounts
  absorb this on iteration; a clean-image CI build eats it once.

## Decisions (resolved at scope time — do not re-litigate unless reality contradicts)

- **Base image = `ubuntu:22.04`, pinned by digest.** The target is Ubuntu desktops;
  building *on* the minimum LTS line users actually run (22.04 — the floor that ships
  webkit2gtk-4.1) guarantees the runtime contract holds on the same line we tell users
  to install. Rejected `debian:bookworm-slim`: the ~50 MB size saving is irrelevant on a
  multi-GB builder image, and Ubuntu alignment is worth more than a slimmer base.
- **CI extraction = multi-stage `FROM scratch` artifact stage** via
  `docker build --output type=tar,dest=-`. One command, fully reproducible from a clean
  checkout, no follow-up `cp` to forget — the normal way to ship a single binary out of
  a build image. Rejected `docker create` + `docker cp`: two-step, leaky (the build
  container lingers), and the older pattern.
- **Dev invocation runs as the host UID/GID** (`--user $(id -u):$(id -g)`, with HOME
  and the cargo/pnpm cache dirs pre-created writable by that UID in the image). Bind-
  mounted files (`target/`, `node_modules/`) come out owner-correct by construction —
  no `sudo chown` after every build. Rejected root + chown: it's the old hack and the
  devx cost (root-owned files littering the working tree) is real.
- **Same image runs the xvfb smoke.** One box builds and verifies — "it built" and "it
  boots" stay coupled, the failure mode the packaging scope names (binary opens a
  window) is caught in the same reproducible environment that produced it. Rejected a
  separate smoke step that downloads the artifact: it splits the proof across two
  environments and re-introduces the host-toolchain assumption.
- **Scope is executable-only.** The Dockerfile ships the bare-binary path (the parent
  slice's deliverable); deb/rpm/AppImage are added when their `desktop/build/linux/<type>/`
  dirs land, by adding a `--bundles <type>` job on the **same** image — not by forking
  it. Rejected pre-modelling the installer types now: speculative breadth the parent
  scope doesn't ship.

## Skill doc

Yes — `docs/skills/desktop-build-container/SKILL.md`, grounded in a live image build:
the `docker build` one-liner, the two `docker run` invocations (dev bind-mount + CI
extract), the cache volume names, the host-UID gotcha, where the binary lands, and the
in-container smoke command. Drivable by an agent or a new contributor verbatim.

## Related

- `docs/scope/desktop/desktop-packaging-scope.md` — the parent slice; this is its
  "real dev box or CI" line made reproducible.
- `desktop/README.md` + `desktop/docs/linux/README.md` — the workspace this image
  lives in; the latter's build section becomes one `docker run` once this ships.
- `desktop/build/linux/` — the per-output-type dirs (deb/rpm/AppImage); the same image
  will build each when its slice lands.
- `ui/src-tauri/` (`Cargo.toml`, `tauri.conf.json`, `build.rs`, `src/desktop.rs`) — the
  shell crate the image compiles.
- `rust/.cargo/config.toml` — the zig-linker hack the regression test proves does NOT
  leak into the container.
- `pnpm-workspace.yaml` (repo root) — why the image must see the whole repo, not just
  `ui/`.
- `docker/` (repo root) — runtime node images; this builder is separate and build-only.
- `.github/workflows/ci.yml` — the toolchain versions (Node 22, pnpm 9, stable Rust)
  the image matches; the future `desktop-build.yml` lane consumes the image.
- README §5 (`workstation` persona), §3 rule 1 (symmetric nodes), §6.13 (Frontend /
  UI delivery).
