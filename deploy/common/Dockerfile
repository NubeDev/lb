# deploy/common/Dockerfile — the ONE image every driver (local Docker, GitHub CI, Fly.io) builds
# from. See ../README.md and docs/scope/deploy/fly-deploy-scope.md. Build context is the REPO ROOT
# (`lb/`) — everything the workspace needs (rust/) lives under it; there is no sibling-dir path-dep
# the way dev-pulse needed `../starter`.
#
# HEADLESS. This image is the lb NODE, not a product. lb is a library (MIGRATION.md) and `ui/` was
# deleted (commit 678503f) — the SPA-builder stage that used to live here built `lazybones-ui` from
# that tree and had been failing every build since. A product host (rubix-ai) vendors its own shell
# and builds its own image FROM this one (or copies the binaries); serving a SPA is its job, not
# lb's. Caddy stays as the single :8080 front door so that contract is unchanged for callers.
#
# Two stages: build the Rust binaries (node + federation sidecar), then assemble a debian-slim
# runtime with Caddy + tini fronting the gateway.

# ============================================================ rust
FROM rust:1-bookworm AS rust-builder

# openssl-sys (federation's `postgres` feature, vendored build) needs a C toolchain + perl;
# rusqlite's `bundled` feature needs a C compiler too. wasm32-wasip2 is the `hello` guest target.
RUN apt-get update && apt-get install -y --no-install-recommends \
      pkg-config libssl-dev ca-certificates perl make \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add wasm32-wasip2

WORKDIR /src
COPY rust /src/rust

# The `hello` wasm guest — the node reads it at startup (main.rs). Built first since the host
# build below does not depend on it, but the runtime image needs the artifact either way.
WORKDIR /src/rust/extensions/hello
RUN cargo build --target wasm32-wasip2 --release

# The host workspace: node (release) + the federation sidecar (release, postgres feature — the
# headline datasource; see extensions/federation/Cargo.toml). One `cargo build` invocation so the
# dependency graph is resolved once.
WORKDIR /src/rust
RUN cargo build --release -p lb-node -p federation --features federation/postgres \
 && cp target/release/node /tmp/node \
 && cp target/release/federation /tmp/federation

# ============================================================ runtime
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates tini curl gnupg debian-keyring debian-archive-keyring \
      apt-transport-https jq python3 \
 && curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
      | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg \
 && curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
      > /etc/apt/sources.list.d/caddy-stable.list \
 && apt-get update && apt-get install -y --no-install-recommends caddy \
 && apt-get purge -y gnupg apt-transport-https \
 && apt-get autoremove -y \
 && rm -rf /var/lib/apt/lists/*

# The two Rust binaries.
COPY --from=rust-builder /tmp/node        /usr/local/bin/node
COPY --from=rust-builder /tmp/federation  /usr/local/bin/federation

# The `hello` wasm guest + its `extension.toml`, at the EXACT path `main.rs`'s
# `env!("CARGO_MANIFEST_DIR")/../extensions/hello` resolves to at RUN time, because
# `CARGO_MANIFEST_DIR` is baked in at compile time as the builder's absolute path
# (/src/rust/node). The manifest read is unconditional (no env override) so we mirror the exact
# builder layout here rather than relocating it — no extra env needed for either file to resolve.
# `/src/rust/node` itself must also exist (even empty): the compiled-in path is
# `.../node/../extensions/hello/...`, and `openat` fails ENOENT on a missing INTERMEDIATE
# component even though the target directory it resolves to is present.
RUN mkdir -p /src/rust/node
COPY --from=rust-builder /src/rust/extensions/hello/extension.toml \
     /src/rust/extensions/hello/extension.toml
COPY --from=rust-builder /src/rust/extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm \
     /src/rust/extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm

# The Docker-free demo dataset generator + registration script (datasources scope) — the entrypoint
# runs this once to seed a real `kind:sqlite` datasource so the Datasources page/Data Studio have
# something to show on first boot. stdlib-only in --sqlite mode (no pip installs needed).
COPY docker/postgres/seed.py docker/postgres/generators.py docker/postgres/inventory.py \
     docker/postgres/tags.py docker/postgres/sinks_sqlite.py docker/postgres/seed-demo-sqlite.sh \
     /opt/lazybones/seed/
RUN chmod +x /opt/lazybones/seed/seed-demo-sqlite.sh

# The web root Caddy serves static files from. Empty in this image by design — a product host
# mounts or COPYs its own SPA bundle here. It must EXIST though: Caddy's `file_server` errors on a
# missing root, and an empty dir yields a clean 404 instead.
RUN mkdir -p /usr/share/lazybones/web

COPY deploy/common/Caddyfile     /etc/caddy/Caddyfile
COPY deploy/common/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

EXPOSE 8080
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/entrypoint.sh"]
