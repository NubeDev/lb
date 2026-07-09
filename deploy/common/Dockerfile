# deploy/common/Dockerfile — the ONE image every driver (local Docker, GitHub CI, Fly.io) builds
# from. See ../README.md and docs/scope/deploy/fly-deploy-scope.md. Build context is the REPO ROOT
# (`lb/`) — everything the workspace needs (rust/, ui/, packages/) lives under it; there is no
# sibling-dir path-dep the way dev-pulse needed `../starter`.
#
# Three stages: build the Rust binaries (node + federation sidecar), build the React SPA, then
# assemble a debian-slim runtime with Caddy + tini fronting both.

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
RUN cargo build --release -p node -p federation --features federation/postgres \
 && cp target/release/node /tmp/node \
 && cp target/release/federation /tmp/federation

# ============================================================ spa
FROM node:22-slim AS spa-builder

RUN npm install -g pnpm@10

WORKDIR /src
COPY pnpm-workspace.yaml pnpm-lock.yaml /src/
COPY packages /src/packages
COPY ui /src/ui
# `tsc --noEmit` (in `ui`'s build script) type-checks test files that import generated JSON
# straight out of the Rust tree by relative path (e.g. widgetCatalog.consistency.test.ts →
# ../../../../../rust/crates/host/src/dashboard/widget_catalog.json) to keep the UI catalog and
# the host's compiled-in catalog from drifting — so this stage needs `rust/` on disk too, even
# though it builds nothing from it.
COPY rust /src/rust

# Same-origin SPA (README rule 1 posture: config, not a code branch): built with an EMPTY
# VITE_GATEWAY_URL so every API call is a relative same-origin request that Caddy proxies to the
# node — see ui/src/lib/ipc/http.ts `gatewayUrl()`.
ENV VITE_GATEWAY_URL=""
RUN pnpm install --frozen-lockfile=false \
 && pnpm --filter @nube/nav-rail run build \
 && pnpm --filter lazybones-ui run build

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

# The SPA bundle, served by Caddy.
COPY --from=spa-builder /src/ui/dist /usr/share/lazybones/web

COPY deploy/common/Caddyfile     /etc/caddy/Caddyfile
COPY deploy/common/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

EXPOSE 8080
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/entrypoint.sh"]
