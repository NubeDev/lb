#!/usr/bin/env bash
# deploy/fly/smoke.sh — boot smoke test against a RUNNING container (local compose or a live Fly
# app). Real HTTP calls against a real node (rule 9: no mocks) — see docs/scope/deploy/
# fly-deploy-scope.md "Testing plan".
#
# Usage: bash deploy/fly/smoke.sh [BASE_URL] [EMAIL] [WORKSPACE] [PASSWORD]
#   BASE_URL defaults to http://127.0.0.1:8080 (deploy/common/compose.yml's published port).
#   EMAIL/PASSWORD match the boot seed's LB_SEED_EMAIL/LB_SEED_PASSWORD defaults.
set -euo pipefail

BASE="${1:-http://127.0.0.1:8080}"
EMAIL="${2:-ada@acme.local}"
WS="${3:-acme}"
PASSWORD="${4:-dev-admin-pw}"

pass() { echo "  ok: $1"; }
fail() { echo "  FAIL: $1"; exit 1; }

echo "-> SPA served at $BASE/"
code=$(curl -fsS -o /dev/null -w '%{http_code}' "$BASE/")
[ "$code" = "200" ] && pass "GET / -> 200" || fail "GET / -> $code"

echo "-> login as $EMAIL"
TOKEN=$(curl -fsS -X POST "$BASE/auth/login" -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\"}" | jq -r .token)
[ -n "$TOKEN" ] && [ "$TOKEN" != "null" ] || fail "login did not return a token"
pass "POST /auth/login -> token"

echo "-> gateway reachable through Caddy (same-origin proxy, not the SPA fallback)"
ws_body=$(curl -fsS "$BASE/workspaces" -H "authorization: Bearer $TOKEN")
echo "$ws_body" | jq -e ".[] | select(.ws==\"$WS\")" >/dev/null \
  || fail "GET /workspaces did not include '$WS' (got: $ws_body)"
pass "GET /workspaces includes '$WS'"

echo "-> demo datasource seeded and queryable"
ds_body=$(curl -fsS -X POST "$BASE/mcp/call" -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"tool":"datasource.list","args":{}}')
echo "$ds_body" | jq -e '.datasources[] | select(.name=="demo-buildings")' >/dev/null \
  || fail "datasource.list did not include 'demo-buildings' (got: $ds_body)"
pass "datasource.list includes 'demo-buildings'"

echo "smoke: all checks passed against $BASE"
