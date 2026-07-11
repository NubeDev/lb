#!/usr/bin/env bash
# Build the `federation` extension — the native (Tier-2) datasources extension (datasources scope).
# A host-platform binary AND a workspace member, so it builds for the host target via the shared
# workspace target/ dir. Produces:
#   rust/target/release/federation   (the binary the host supervisor spawns over stdio).
set -euo pipefail
cd "$(dirname "$0")"
# -p against the workspace so it shares the workspace lockfile and target dir.
# The HEADLINE source is Postgres/Timescale, gated behind the `postgres` feature (off by default in
# Cargo.toml because it pulls native-tls → openssl). Build it ON here so the shipped binary can
# actually connect to a Postgres source — without it, every postgres/timescale call returns
# "postgres source not built in" (sqlite-only). Requires a C toolchain (openssl/vendored).
# Set FEDERATION_NO_POSTGRES=1 to fall back to the sqlite-only build where no TLS toolchain exists.
if [[ "${FEDERATION_NO_POSTGRES:-}" == "1" ]]; then
  cargo build --release -p federation
  echo "built: federation (sqlite-only — postgres feature OFF)"
else
  cargo build --release -p federation --features postgres
  echo "built: federation (workspace target/release/federation, +postgres)"
fi
