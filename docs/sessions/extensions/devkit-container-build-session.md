# Session — hermetic devkit container builds

Scope: `docs/scope/extensions/devkit-container-build-scope.md`.

## What shipped

- `ContainerToolchain` (`rust/crates/devkit/src/container_toolchain.rs`) — a second
  `Toolchain` impl (alongside `ProcessToolchain`) that runs `cargo`/`pnpm` inside the
  pinned `docker/build/` image via `docker run`, instead of as a bare child of the node
  process. Same trait, same `build_extension` call, same job/log/publish contract —
  `devkit.build` doesn't know which executor ran.
- Toolchain selection by **config, not branch**: `LB_DEVKIT_BUILDER=container` opts in
  (`rust/crates/host/src/devkit/builder.rs::select_toolchain`, wired at
  `rust/crates/host/src/devkit/build.rs:41`); unset keeps the existing in-process fast
  path.
- A build-scoped git token, read from `lb-secrets` at `devkit/build-git-token` under a
  host-mediated `ext:devkit` principal (never the caller's authority — same shape as
  `federation::secret::mediate_dsn`), injected into the container as `LB_BUILD_GIT_TOKEN`
  and consumed by a git credential helper baked into the image
  (`docker/build/git-credential-lb-build.sh`) — never a tokenized URL, never in the
  streamed log.
- `docker/build/Dockerfile` extended (same image as the existing cross-build toolchain,
  per the scope's "one image" call): Node 20 + pnpm for the UI build step, the credential
  helper, and running the whole image as the **host uid/gid** (`-u`) so build output
  under the mounted extension tree comes back owned by the caller, not root. Dropped the
  image's `ENTRYPOINT` (it forced every `docker run` through `lb-build`, incompatible
  with `ContainerToolchain` running `cargo`/`pnpm` directly as the command); the
  cross-build Makefile target now passes `lb-build <target>` explicitly.
- `ContainerToolchain::run` mounts the whole `rust/` **workspace root** (discovered by
  walking up for the ancestor `Cargo.toml` with a real `[workspace] members = [...]`),
  not just the extension subtree — a generated extension's `Cargo.toml` has
  `path = "../../crates/..."` deps that escape its own directory (devkit templates are
  intentionally not workspace members), so mounting only the extension broke those deps.
- Fixed stdout/stderr interleaving in **both** `ProcessToolchain::run` and
  `ContainerToolchain::run` (previously `ProcessToolchain` drained all of stdout before
  touching stderr) — a failing `cargo`'s stderr line now shows up where it happened, not
  buried after the full download log.

## Why

Building `control-engine` (a private-git-dep extension) from Extension Studio failed
with `exit status: 101` — the node process inherited VS Code's `GIT_ASKPASS`, which can't
authenticate a spawned child's git fetch. The same build succeeded from the user's own
shell. See the debug entry for the full investigation:
`docs/debugging/extensions/devkit-build-fails-exit-101-private-git-dep.md`.

## Testing

`rust/crates/host/tests/devkit_container_build_test.rs` (real store, real caps, real
Docker CLI — no mocks):

- `builder_config_selects_process_by_default` / `builder_config_container_flag_parses` —
  fallback selection (unit).
- `container_toolchain_builds_same_artifact_as_process_toolchain` — toolchain-parity
  (integration): the same generated native extension, with its real `path`-dependency on
  `lb-supervisor`, builds to an installable artifact via `ContainerToolchain`. Skips (not
  fails) if Docker or the `lazybones-build` image isn't present.
- `container_build_log_never_contains_the_git_token` — the private-dep credential test:
  seeds a real `lb-secrets` entry and asserts the streamed build log never contains the
  token bytes. This is the regression test for the `control-engine exit 101` symptom.
- `container_build_fails_clearly_when_image_is_missing` — deny/failure path: a
  misconfigured image name fails the build cleanly, not a panic or hang.

Capability-deny and workspace-isolation for `devkit.build` were already covered
executor-agnostically by the pre-existing `rust/crates/host/tests/devkit_test.rs`
(`each_devkit_mcp_verb_denies_without_its_grant`, `build_job_record_is_workspace_scoped`)
— the toolchain swap happens strictly after those gates, so no new coverage was needed
there.

Green: `cargo test -p lb-devkit -p lb-host --test devkit_test --test devkit_e2e_test
--test devkit_container_build_test` (12/12), `cargo build --workspace`,
`cargo fmt --all -- --check`.

## Follow-up (2026-07-03): host `.cargo/config.toml` zig linker leaked into the image

First real container build after `make docker-build-image` failed with
`linker /home/user/.local/bin/zigcc not found` — the mounted `rust/` workspace carries a
host-only `.cargo/config.toml` pinning the x86_64 linker + `CC/AR/RANLIB` at this box's
personal zig shims, which don't exist in the image. `ContainerToolchain` runs `cargo`
directly (not via `lb-build`), so it never applied the `--config`/CC overrides that the
cross-build path uses. Fix: `ContainerToolchain::run` now exports real-GCC `CC/AR/RANLIB`
via `-e` and, for `cargo`, injects
`--config=target.x86_64-unknown-linux-gnu.linker="x86_64-linux-gnu-gcc"` (a config
`linker` can't be beaten by env — only by `--config`). Debugged in
`../../debugging/extensions/devkit-container-build-inherits-host-zig-linker.md`. Verified:
the exact failing `cargo build --target wasm32-wasip2 --release` now finishes in-container
(artifact host-uid-owned) and `devkit_container_build_test` stays 5/5 green — the native
parity test is the regression guard (host-triple link is the path that used to break).

## Open follow-ups (from the scope doc, unresolved by design)

- Runtime target is Docker-only for v1 (no Podman abstraction yet).
- Cache volume is one shared `lazybones-cargo-cache` volume per node (registry/target
  scratch only); per-workspace cache isolation is not implemented.
- Image distribution is local-build-only (`make docker-build-image`); no pinned
  published tag yet.
