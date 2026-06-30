# docker/build/ — reproducible Linux cross-build for the `node` binary

A clean Docker toolchain so building never depends on whatever C compiler /
half-installed zig wrapper is on your host. It fixes the
`/home/user/.local/bin/zigranlib: not found` failure: the `federation` crate
vendors OpenSSL (compiles it from C), and a stray zig install was hijacking the
C toolchain. This image uses **real Debian GCC cross-toolchains** instead.

## Quick start

From the repo root:

```sh
make docker-build-image       # build the toolchain image once
make docker-build             # build the node binary for linux x86_64 (default)
```

The binary lands in your normal `rust/target/<triple>/release/` — the container
mounts the repo and shares a named cargo cache volume, so builds are incremental.

## Targets (more over time)

```sh
make docker-build TARGET=linux-x86_64     # x86_64 (default)
make docker-build TARGET=linux-arm64      # aarch64 (64-bit Pi)
make docker-build TARGET=linux-armv7      # armv7 (32-bit Pi)
make docker-build TARGET=windows-x86_64   # node.exe
make docker-build TARGET=deb              # .deb package (target/debian/)

# the federation sidecar (needs vendored OpenSSL — the whole reason for this image):
make docker-build PKG=federation FEATURES=postgres
```

Add a target by editing [`build.sh`](build.sh) (an alias → rust triple) and the
matching cross-package + `rustup target add` in the [`Dockerfile`](Dockerfile).

## How it avoids the zig problem

Each target gets a genuine `*-gcc` / `*-ranlib` from Debian's `gcc-<triple>`
packages, and the Dockerfile sets `CC_<triple>` / `CARGO_TARGET_<T>_LINKER` so
cargo, the `cc` crate, and vendored OpenSSL all use those — never a zig wrapper.
