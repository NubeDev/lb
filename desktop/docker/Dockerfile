# The Linux build environment for the lazybones-shell desktop binary.
# See desktop/docker/README.md and docs/scope/desktop/desktop-build-container-scope.md.
#
# One Dockerfile, three BuildKit stages, one build context (the repo root):
#   base     — toolchain only. Dev: `--target base`, then bind-mount the repo and `docker run`.
#   builder  — base + COPY repo + RUN build. Reached transitively by `--target artifact`.
#   artifact — FROM scratch, the binary only. CI: `--target artifact --output type=tar`.
#
# Always build from the repo root so `COPY . /build` (builder stage) and the repo-root
# `.dockerignore` apply:
#   docker build -f desktop/docker/Dockerfile --target <base|artifact> .

# ─── base: Ubuntu 22.04 — the minimum LTS line that ships webkit2gtk-4.1 ──────────
# Pinning the base fixes the webkit soname; bumping it shifts the runtime contract's
# minimum distro line. See the scope's Decisions section.
FROM ubuntu:22.04 AS base
ARG DEBIAN_FRONTEND=noninteractive

# The webkit2gtk-4.1 toolchain the packaging scope names (Tauri v2 requirement — not 4.0),
# plus xvfb for the in-container smoke and the usual build niceties.
RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential \
      pkg-config \
      libwebkit2gtk-4.1-dev \
      libgtk-3-dev \
      libsoup-3.0-dev \
      libjavascriptcoregtk-4.1-dev \
      librsvg2-dev \
      ca-certificates \
      curl \
      git \
      file \
      xvfb \
    && rm -rf /var/lib/apt/lists/*

# Rust stable — matches .github/workflows/ci.yml (dtolnay/rust-toolchain@stable).
# CARGO_HOME is relocated out of $HOME so the cache lives at a known, mountable path
# and survives dev iterations via a named volume.
ENV CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    PATH="/usr/local/cargo/bin:${PATH}"
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
  | sh -s -- -y --default-toolchain stable --profile minimal --no-modify-path

# Node 22 + pnpm 9 — matches .github/workflows/ci.yml exactly (node-version: 22, pnpm 9).
# PNPM_HOME relocated out of $HOME for the same reason as CARGO_HOME.
ENV PNPM_HOME=/usr/local/pnpm \
    PATH="/usr/local/pnpm:${PATH}"
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
 && apt-get install -y --no-install-recommends nodejs \
 && rm -rf /var/lib/apt/lists/* \
 && npm install -g pnpm@9 \
 && pnpm config set store-dir /usr/local/pnpm/store

# Cache + HOME dirs pre-created world-writable so dev mode `--user $(id -u):$(id -g)`
# Just Works for arbitrary host UIDs (the OpenShift arbitrary-UID convention). Build
# output lands in the bind-mounted repo's target/ — owner-correct by construction
# because container UID == host UID. Only the cache roots and HOME need this; the
# rustup/node toolchain above stays root-owned and world-readable.
RUN mkdir -p /usr/local/cargo/registry /usr/local/cargo/git \
             /usr/local/pnpm/store \
             /home/builder \
 && chmod -R 0777 /usr/local/cargo/registry /usr/local/cargo/git \
                  /usr/local/pnpm \
                  /home/builder
ENV HOME=/home/builder

WORKDIR /build
COPY desktop/docker/build.sh /usr/local/bin/build.sh
RUN chmod +x /usr/local/bin/build.sh

# Default: build the shell. Override (`docker run ... <cmd>`) for the in-container smoke.
ENTRYPOINT ["/usr/local/bin/build.sh"]

# ─── builder: COPY repo + RUN build (reached transitively by --target artifact) ──
# CI reproducible path: no bind-mount, the repo is baked in, the build is deterministic
# from a clean checkout.
FROM base AS builder
COPY . /build
RUN /usr/local/bin/build.sh

# ─── artifact: the binary only — `docker build --target artifact --output type=tar` ─
# Scratch base = zero distro, just the ELF. `--output type=tar,dest=-` pipes it out as
# the CI artifact; no `docker cp` follow-up.
FROM scratch AS artifact
COPY --from=builder /build/ui/src-tauri/target/release/lazybones-shell /lazybones-shell
