#!/bin/sh
# entrypoint.sh — boots a container running Caddy + the Lazybones `node` (cloud posture).
#
# The node is entirely env-driven (no config.toml — see rust/node/src/main.rs / federation.rs):
# this script's whole job is to set the right env vars, ensure the volume-backed dirs exist, start
# Caddy, start the node, and (idempotently) seed the demo SQLite datasource once the gateway answers.
#
# Secrets are NOT baked into the image — `LB_SIGNING_KEY` etc. arrive as real env vars (Fly
# secrets / compose env), read directly by the node. Nothing here substitutes them into a file.
set -eu

DATA_DIR=/data
STORE_DIR="$DATA_DIR/store"
DEMO_DB="$DATA_DIR/demo/buildings.db"

mkdir -p "$STORE_DIR" "$DATA_DIR/demo"

: "${LB_GATEWAY_ADDR:=127.0.0.1:8731}"
: "${LB_GATEWAY_URL:=http://127.0.0.1:8731}"
: "${LB_WORKSPACE:=acme}"
: "${LB_SEED_USER:=user:ada}"
: "${LB_STORE_PATH:=$STORE_DIR/node-store}"
# Datasources (federation sidecar): SQLite-only by default (no bundled/hosted Postgres — rule 2).
# `127.0.0.1:0` is the convention endpoint kind=sqlite sources register under; pre-approving it is
# what lets the seed step below register the demo file as a real datasource.
: "${LB_FEDERATION_ENDPOINTS:=127.0.0.1:0}"
: "${LB_FEDERATION_DIR:=/usr/local/bin}"

export LB_GATEWAY_ADDR LB_GATEWAY_URL LB_WORKSPACE LB_SEED_USER LB_STORE_PATH \
       LB_FEDERATION_ENDPOINTS LB_FEDERATION_DIR

echo "[entrypoint] starting Caddy on :8080"
caddy run --config /etc/caddy/Caddyfile --adapter caddyfile &
caddy_pid=$!

echo "[entrypoint] starting node on $LB_GATEWAY_ADDR (ws=$LB_WORKSPACE, store=$LB_STORE_PATH)"
/usr/local/bin/node &
node_pid=$!

trap 'kill -TERM "$node_pid" "$caddy_pid" 2>/dev/null || true' INT TERM

# Best-effort, idempotent demo-datasource seed: don't fail boot if it doesn't land (a slow first
# start, or a rerun where it's already registered) — the node is the thing that must stay up.
(
  for _ in $(seq 1 30); do
    curl -sS -o /dev/null "http://127.0.0.1:8731/workspaces" 2>/dev/null && break
    sleep 1
  done
  if [ ! -s "$DEMO_DB" ]; then
    echo "[entrypoint] seeding demo datasource '$DEMO_DB'"
    /opt/lazybones/seed/seed-demo-sqlite.sh "$DEMO_DB" "$LB_GATEWAY_URL" "$LB_SEED_USER" "$LB_WORKSPACE" \
      || echo "[entrypoint] demo datasource seed failed (non-fatal) — retry with: fly ssh console"
  fi
) &

wait "$node_pid"
