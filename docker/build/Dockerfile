# docker/build/Dockerfile — a clean, reproducible Linux cross-build toolchain for the
# `node` binary (and any workspace crate).
#
# Why this exists: the `federation` crate vendors OpenSSL (`openssl = { features =
# ["vendored"] }`), which compiles OpenSSL from C source at build time. That needs a
# real C toolchain — `cc` + `ar` + `ranlib` — for the TARGET. On a dev box where a
# half-installed `cargo-zigbuild`/zig wrapper hijacks the linker, the build dies with
# `/home/user/.local/bin/zigranlib: not found`. This image side-steps zig entirely:
# it installs Debian's genuine GCC cross-toolchains, so each target gets a real
# `*-gcc`/`*-ranlib` and vendored OpenSSL compiles the normal way.
#
# One image, many targets over time: the toolchains for every supported target are
# baked in; `build.sh` picks one per invocation via the TARGET arg (see that script
# and the Makefile `docker-*` targets). Add a target by adding its Debian cross
# package + the rustup target here, then a stanza in build.sh.
FROM rust:1-bookworm

# Real C cross-toolchains (NOT zig). Each `gcc-<triple>` brings its own cc/ar/ranlib,
# which is exactly what vendored OpenSSL + the linker need per target.
#   - build-essential        native x86_64 host compiler (and the default target)
#   - perl, make             vendored OpenSSL's own build system
#   - gcc-aarch64-*          linux arm64 (aarch64)
#   - gcc-arm-linux-gnueabihf armv7 (hard-float) — the Raspberry-Pi-class edge target
#   - gcc-mingw-w64-x86-64   windows x86_64 (gnu) — for the .exe target
#   - dpkg-dev, fakeroot     building .deb packages (cargo-deb)
RUN dpkg --add-architecture armhf \
 && apt-get update && apt-get install -y --no-install-recommends \
      build-essential perl make pkg-config \
      gcc-aarch64-linux-gnu g++-aarch64-linux-gnu \
      gcc-arm-linux-gnueabihf g++-arm-linux-gnueabihf \
      gcc-mingw-w64-x86-64 \
      dpkg-dev fakeroot \
      ca-certificates \
 && rm -rf /var/lib/apt/lists/*

# rustup targets for everything build.sh can emit. wasm32-wasip2 is here too so the
# image can also build the wasm guests the node loads at startup.
RUN rustup target add \
      x86_64-unknown-linux-gnu \
      aarch64-unknown-linux-gnu \
      armv7-unknown-linux-gnueabihf \
      x86_64-pc-windows-gnu \
      wasm32-wasip2

# cargo-deb produces a .deb from the workspace metadata; only needed for the `deb`
# target but cheap to bake in.
RUN cargo install cargo-deb --locked

# Tell cargo which linker + C compiler to use per target. Cargo reads CARGO_TARGET_<T>_LINKER
# and the `cc` crate reads CC_<triple>; setting both makes the native-tls/openssl-sys C
# build and the final link both use the real GCC cross-tools, never a stray zig wrapper.
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++ \
    AR_aarch64_unknown_linux_gnu=aarch64-linux-gnu-ar \
    CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc \
    CC_armv7_unknown_linux_gnueabihf=arm-linux-gnueabihf-gcc \
    CXX_armv7_unknown_linux_gnueabihf=arm-linux-gnueabihf-g++ \
    AR_armv7_unknown_linux_gnueabihf=arm-linux-gnueabihf-ar \
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
    CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc \
    CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++ \
    AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar

WORKDIR /work
COPY build.sh /usr/local/bin/lb-build
RUN chmod +x /usr/local/bin/lb-build
ENTRYPOINT ["/usr/local/bin/lb-build"]
