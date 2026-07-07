#!/usr/bin/env bash
# The Docker-free demo dataset (sqlite-datasource-demo scope): generate the demo building dataset
# into ONE SQLite file (seed.py --sqlite — lite profile: 1 month @ 15-min), then register it as a
# first-class `kind:"sqlite"` datasource through the NORMAL admin verb (`datasource.add`) on a
# RUNNING node — real records, real engine, no container (rule 9: the anti-mock).
#
# The DSN for a sqlite source is the database FILE PATH, resolved on the node running the
# federation sidecar (not the browser). This script writes the file under the node's own data dir
# so the sidecar can see it. A sqlite source has no network endpoint; it is registered at the
# `127.0.0.1:0` convention, which `make dev`'s default FED_ENDPOINTS pre-approves.
#
# Prereqs: a running `make dev` node with the federation sidecar (default) + jq + curl + python3.
# Usage:  bash docker/postgres/seed-demo-sqlite.sh [DB_PATH] [GATEWAY_URL] [USER] [WORKSPACE]
set -eu
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"

DB="${1:-$ROOT/.lazybones/data/demo/buildings.db}"
GW="${2:-http://127.0.0.1:8080}"
USER="${3:-user:ada}"
WS="${4:-acme}"
NAME="demo-buildings"

command -v jq >/dev/null || { echo "seed-demo-sqlite needs jq"; exit 1; }

echo "-> generating demo dataset (lite profile) into $DB"
python3 "$HERE/seed.py" --sqlite "$DB"

echo "-> login $GW as $USER/$WS"
TOKEN=$(curl -fsS -X POST "$GW/login" -H 'content-type: application/json' \
  -d "{\"user\":\"$USER\",\"workspace\":\"$WS\"}" | jq -r .token)
[ -n "$TOKEN" ] && [ "$TOKEN" != "null" ] || { echo "login failed (is $USER a member of $WS?)"; exit 1; }

echo "-> register datasource '$NAME' (kind sqlite, dsn = node-local file path)"
curl -fsS -X POST "$GW/mcp/call" -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d "{\"tool\":\"datasource.add\",\"args\":{\"name\":\"$NAME\",\"kind\":\"sqlite\",\"endpoint\":\"127.0.0.1:0\",\"dsn\":\"$DB\",\"ts\":$(date +%s)}}" \
  >/dev/null

echo "-> done: '$NAME' registered in '$WS'. Datasources page should probe green;"
echo "   Data Studio's source picker now lists it (tables: site/meter/point/point_reading + tags)."
