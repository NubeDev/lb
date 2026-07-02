# Studio `devkit.build` of `control-engine` dies with `exit status: 101` fetching a private git dep

- Area: extensions
- Status: resolved
- First seen: 2026-07-02
- Resolved: 2026-07-02
- Session: ../../sessions/extensions/devkit-container-build-session.md
- Regression test: rust/crates/host/tests/devkit_container_build_test.rs (`container_build_log_never_contains_the_git_token`, `container_toolchain_builds_same_artifact_as_process_toolchain`)

## Symptom

Building `control-engine` from the Extension Studio (`devkit.build`) fails right after
the crate download step with `exit status: 101` on the streamed `devkit/build/<job_id>`
log — no further detail. The **same** `cargo build` succeeds from the user's own shell
against the same checkout.

## Reproduce

1. Have a private git dependency in an extension's `Cargo.toml` (`control-engine`
   depends on `NubeIO/ce-client-rust`, a private repo).
2. Click **Build** in Extension Studio (or call `devkit.build`) from a VS Code-launched
   gateway node.
3. The build fails at the `cargo` fetch step for the private dependency; the log shows
   only `exit status: 101`.

## Investigation

- The node process (`devkit_build` → `build_extension` → `ProcessToolchain::run`,
  `rust/crates/devkit/src/toolchain.rs`) shells `cargo` out as a **bare child of the
  node process**, inheriting whatever environment that process happens to have.
- The node was launched from inside VS Code, which sets `GIT_ASKPASS` to its own
  credential-prompt helper. `cargo`'s git fetch invokes that helper for the private
  remote; it can't authenticate non-interactively from a spawned child, so the checkout
  fails.
- The user's own shell has no such `GIT_ASKPASS` override (or has a working one), so the
  identical `cargo build` succeeds there — confirming the failure was **environment
  inheritance**, not the crate or the credentials themselves.
- `ProcessToolchain::run` also drained all of stdout before touching stderr, so the
  actual git error (on stderr) was buried after the whole download log — looked like the
  build "just stopped".

## Root cause

`devkit.build` had no boundary between the node process's inherited environment
(`PATH`, `GIT_ASKPASS`, `CARGO_HOME`) and the build. Fetching a private dependency
depends on the operator's shell state, which is not reproducible — it happens to work
in one terminal and fail in another, and fails outright on a cloud node with no host
toolchain at all.

## Fix

Added a hermetic executor behind the existing `Toolchain` trait seam
(`docs/scope/extensions/devkit-container-build-scope.md`):

- `ContainerToolchain` (`rust/crates/devkit/src/container_toolchain.rs`) runs
  `cargo`/`pnpm` inside the pinned `docker/build/` image instead of as a bare child —
  no inherited `GIT_ASKPASS`/`PATH`, only what the image and an explicit `-e` env carry.
- A build-scoped git token is read from `lb-secrets` (`devkit/build-git-token`) and
  injected via a git credential helper baked into the image
  (`docker/build/git-credential-lb-build.sh`), reading `LB_BUILD_GIT_TOKEN` — never a
  tokenized URL, never in the streamed log.
- Selected by config, not a branch: `LB_DEVKIT_BUILDER=container` opts in
  (`rust/crates/host/src/devkit/builder.rs::select_toolchain`); unset keeps the existing
  `ProcessToolchain` fast inner loop.
- Also fixed the stdout/stderr interleaving in both `ProcessToolchain::run` and
  `ContainerToolchain::run` so a failing `cargo`'s stderr line appears where it happened,
  not after the full stdout dump.

## Verification

`cargo test -p lb-host --test devkit_container_build_test` — 5/5 green, including a
container build of a native extension with a `path`-dependency on the real workspace
crates (proving the mount covers `../../crates/...` deps the same way the host shell
does), and a credential test that seeds a real `lb-secrets` token and asserts the
streamed log never contains it.

## Prevention

Regression tests: `container_toolchain_builds_same_artifact_as_process_toolchain`
(toolchain-parity) and `container_build_log_never_contains_the_git_token` (credential
never logged). The container path structurally can't inherit `GIT_ASKPASS`/`PATH` from
the node process — the class of bug (spawn-inherits-operator-shell) is closed for any
future devkit build, not just this one dependency.
