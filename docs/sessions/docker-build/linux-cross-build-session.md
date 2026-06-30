# Session: Dockerized Linux cross-build for the `node` binary

## The ask

Host builds were failing with `/home/user/.local/bin/zigranlib: not found` while
compiling vendored OpenSSL (the `federation` crate, `openssl/vendored`). A
half-installed `cargo-zigbuild`/zig toolchain was hijacking the C compiler + linker.
Wanted: a Docker build environment that builds for Linux reliably, and that can grow
to other targets over time (armv7, deb, windows x86).

## What shipped

`docker/build/` — a reproducible cross-build toolchain:

- `Dockerfile` — `rust:1-bookworm` + **real Debian GCC cross-toolchains** (not zig):
  `build-essential` (x86_64), `gcc-aarch64-linux-gnu`, `gcc-arm-linux-gnueabihf`
  (armv7), `gcc-mingw-w64-x86-64` (windows), plus `cargo-deb` and `perl`/`make` for
  vendored OpenSSL's own build. Sets `CC_<triple>` / `CARGO_TARGET_<T>_LINKER` per
  target so cargo, the `cc` crate, and vendored OpenSSL all use the genuine GCC tools.
- `build.sh` (entrypoint `lb-build`) — maps a friendly alias (`linux-x86_64`,
  `linux-arm64`, `linux-armv7`, `windows-x86_64`, `deb`) → rust triple and runs the
  build. `PKG`/`PROFILE`/`FEATURES` env knobs.
- `README.md` — usage.

`Makefile` — `docker-build-image` and `docker-build` targets (default
`TARGET=linux-x86_64`).

## The gotcha (logged in debugging/)

`rust/.cargo/config.toml` is host-specific: that box has no system `cc`, so it pins
the linker/CC/AR/RANLIB at `~/.local/bin/zig*`. That config is mounted into the
container and broke the clean build (`zigcc not found`). Fix: `build.sh` exports the
real `CC`/`AR`/`RANLIB` (cargo config `[env]` without `force` does NOT override a
process env var) and passes `--config target.<triple>.linker="…-gcc"` to beat the
host config's `[target.*].linker`. See
`docs/debugging/build/host-cargo-config-zig-leaks-into-container.md`.

## Use

```sh
make docker-build-image
make docker-build                      # linux x86_64 (default)
make docker-build TARGET=linux-armv7   # 32-bit Pi
make docker-build TARGET=deb           # .deb package
make docker-build PKG=federation FEATURES=postgres
```
