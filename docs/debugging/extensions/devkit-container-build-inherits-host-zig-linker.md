# Container devkit build dies `linker /home/user/.local/bin/zigcc not found`

- Area: extensions
- Status: resolved
- First seen: 2026-07-03
- Resolved: 2026-07-03
- Session: ../../sessions/extensions/devkit-container-build-session.md
- Regression test: rust/crates/host/tests/devkit_container_build_test.rs (`container_toolchain_builds_same_artifact_as_process_toolchain`)

## Symptom

With `LB_DEVKIT_BUILDER=container` and the `lazybones-build` image freshly built, a
`devkit.build` (e.g. `cargo build --target wasm32-wasip2 --release`) starts inside the
container, downloads crates, then dies compiling the first build-script/proc-macro crate:

```
error: linker `/home/user/.local/bin/zigcc` not found
  = note: No such file or directory (os error 2)
error: could not compile `icu_properties_data` (build script) due to 1 previous error
...
docker run (cargo) exited with exit status: 101
devkit build: failed
```

(A separate earlier symptom — `pull access denied for lazybones-build` — is just the
one-time image not being built yet: run `make docker-build-image`. This entry is the
failure that follows *after* the image exists.)

## Reproduce

1. `make docker-build-image` once so `lazybones-build` exists locally.
2. Run a container-mode devkit build of any extension (wasm or native tier).
3. It fails at the first crate that links for the **host** target (`quote`,
   `proc-macro2`, `serde_core`, `icu_*` build scripts) with `zigcc not found`.

## Investigation

- `ContainerToolchain::run` (`rust/crates/devkit/src/container_toolchain.rs`) mounts the
  whole `rust/` workspace at `/work` (needed so a generated extension's
  `path = "../../crates/..."` deps resolve) and runs `cargo` **directly** as the container
  command — not through `docker/build/build.sh` (`lb-build`).
- That mount includes the host's `rust/.cargo/config.toml`, which is deliberately
  host-specific: this box has no system C compiler, so it pins
  `[target.x86_64-unknown-linux-gnu].linker = "/home/user/.local/bin/zigcc"` and
  `[env] CC/AR/RANLIB` at the same personal zig shims.
- Inside the image those paths don't exist (the image has a real GCC cross-toolchain), so
  the host-target link of any build-script/proc-macro crate — compiled for the host triple
  even during a **wasm** guest build — fails immediately.
- `lb-build` never hit this because it neutralizes the same config with explicit
  `--config target.x86_64-unknown-linux-gnu.linker=...` flags and real-GCC `CC/AR/RANLIB`
  exports (see the comments in `docker/build/build.sh` / `Dockerfile`). `ContainerToolchain`
  bypasses `lb-build` and never applied those overrides.

## Root cause

The container executor ran `cargo` against a mounted workspace whose `.cargo/config.toml`
forces a host-only zig linker that doesn't exist in the image. cargo config's `[env]` does
not override a process env var (no `force = true`), and — the key trap — a config
`[target.*].linker` **cannot** be beaten by an env var at all; only a higher-precedence
`--config` flag wins.

## Fix

In `ContainerToolchain::run`:

- Export the image's genuine GCC as `CC/AR/RANLIB` (and the triple-scoped
  `CC_x86_64_unknown_linux_gnu` etc.) via `-e`, beating the config's `[env]` block.
- When `program == "cargo"`, inject
  `--config=target.x86_64-unknown-linux-gnu.linker="x86_64-linux-gnu-gcc"` right after the
  subcommand — the one thing that overrides the config `linker`. Only for `cargo` so
  `pnpm`/other programs are untouched.

This mirrors exactly what `lb-build` already does; the executor now agrees with the
cross-build path on which linker/CC the image uses.

## Verification

- The exact failing invocation, re-run with the fix, compiles `icu_*`/`quote`/`serde_core`
  and finishes `hello-ext` cleanly; the `.wasm` artifact lands host-uid-owned.
- `cargo test -p lb-host --test devkit_container_build_test` — 5/5 green. The parity test
  `container_toolchain_builds_same_artifact_as_process_toolchain` builds a **native**-tier
  extension in the container (host-triple link — the exact path that used to hit
  `zigcc not found`) and matches the `ProcessToolchain` artifact.

## Prevention

The parity regression test now genuinely exercises the container's host-target link path,
so a re-introduction of the leak fails the suite. Any future host-specific `.cargo/config`
key that the image can't honor must be neutralized the same way (`--config` for
config-only keys, `-e` for env-overridable ones).
