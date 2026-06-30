# `zigcc`/`zigranlib: not found` — host `.cargo/config.toml` leaks into the build container

## Symptom

Two related failures, both from the zig toolchain pinned in `rust/.cargo/config.toml`:

On the **host** (`make federation` / any build touching vendored OpenSSL):

```
/home/user/.local/bin/zigranlib: not found
Error installing OpenSSL: 'make' reported failure
```

Inside the new **Docker build** image, before the fix:

```
error: linker `/home/user/.local/bin/zigcc` not found
```

## Cause

`rust/.cargo/config.toml` is **host-specific**. That dev box has no system C
compiler, so it points the linker + `CC`/`AR`/`RANLIB` at a personal zig toolchain
(`~/.local/bin/zigcc`, `zigar`, `zigranlib`). The `federation` crate builds vendored
OpenSSL, whose own Makefile calls `ranlib` directly — when the zig wrapper is missing
or half-installed, `zigranlib: not found` and the OpenSSL build dies.

When that repo is mounted into the clean Docker build image (`docker/build/`), the
same config applies — but `~/.local/bin/zig*` does not exist in the container, so the
linker is "not found".

## Fix

The container has real GCC cross-toolchains, so we override the host config inside
`docker/build/build.sh`:

- Export `CC`/`AR`/`RANLIB` (+ `*_x86_64_unknown_linux_gnu`) to the real GCC tools.
  cargo's config `[env]` block does **not** override an already-set process env var
  unless it sets `force = true` (this one doesn't), so the exports win.
- Pass `--config target.x86_64-unknown-linux-gnu.linker="x86_64-linux-gnu-gcc"` to
  every cargo call — a `[target.*].linker` in config can't be beaten by env alone, so
  a CLI `--config` override is needed.

Cross targets (arm64/armv7/windows) are unaffected by the host config (it only pins
x86_64) and use the per-triple linkers set in the Dockerfile.

## Regression guard

`docker/build/` is itself the guard: `make docker-build` exercises the full
vendored-OpenSSL path in a toolchain with no zig present. If the host config ever
leaks again, that build fails immediately with `zigcc not found`.

The host fix (out of scope here) is to delete the zig block from
`rust/.cargo/config.toml` once a real `gcc`/`clang` is installed on the box.
