#!/usr/bin/env bash
# Seed the thecrew (Graphics) demo into a RUNNING node: the AHU-1 scene doc + its bound `ahu1.*`
# series + a read-only dashboard cell — all through the REAL host verbs (assets.put_doc / ingest /
# dashboard.save), the exact paths production data flows through (no fakes, rule 9). This is the
# "first-run create demo scenes" seed (parent graphics-canvas scope Open question 4): a member with
# the assets.* grant can run it any time; it is idempotent (put_doc/save are UPSERTs).
#
# Prereqs: a running node with `thecrew` PUBLISHED (make publish-ext EXT=thecrew) + `jq` + `curl`.
# Usage:  bash rust/extensions/thecrew/seed-demo.sh [GATEWAY_URL] [USER] [WORKSPACE]
set -eu
GW="${1:-http://127.0.0.1:8080}"
USER="${2:-user:ada}"
WS="${3:-acme}"

command -v jq >/dev/null || { echo "seed-demo needs jq"; exit 1; }
HERE="$(cd "$(dirname "$0")" && pwd)"

echo "-> login $GW as $USER/$WS"
TOKEN=$(curl -fsS -X POST "$GW/login" -H 'content-type: application/json' \
  -d "{\"user\":\"$USER\",\"workspace\":\"$WS\"}" | jq -r .token)
[ -n "$TOKEN" ] && [ "$TOKEN" != "null" ] || { echo "login failed (is $USER a member of $WS?)"; exit 1; }

call() { # tool, json-args  -> POST /mcp/call
  curl -fsS -X POST "$GW/mcp/call" -H "authorization: Bearer $TOKEN" \
    -H 'content-type: application/json' -d "{\"tool\":\"$1\",\"args\":$2}"
}
ingest() { # series, payload
  curl -fsS -X POST "$GW/ingest" -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
    -d "{\"samples\":[{\"series\":\"$1\",\"producer\":\"seed\",\"ts\":1,\"seq\":1,\"payload\":$2}]}" >/dev/null
}

echo "-> seed AHU-1 bound series (ahu1.*)"
ingest "ahu1.sf1.running" true
ingest "ahu1.sf1.speed"   1800
ingest "ahu1.sf1.fault"   false
ingest "ahu1.oad.position" 78
ingest "ahu1.filter.dp"   0.42
ingest "ahu1.chwv.valve"  45
ingest "ahu1.rat"         22.4
ingest "ahu1.sat"         14.1

echo "-> put scene:ahu-1 (assets.put_doc, content_type json, tag scene)"
CONTENT=$(jq -Rs . < "$HERE/docs/ahu-1.scene.json")
call assets.put_doc \
  "{\"id\":\"scene:ahu-1\",\"title\":\"AHU-1\",\"content\":$CONTENT,\"content_type\":\"json\",\"tags\":[\"scene\"],\"ts\":1}" \
  >/dev/null

echo "-> save 'Graphics Scene' dashboard (a read-only ext:thecrew/scene cell + sceneId var)"
call dashboard.save '{
  "id":"scene-dash","title":"Graphics Scene","now":1,
  "variables":[{"name":"sceneId","type":"const","const":"scene:ahu-1"}],
  "cells":[{"i":"c1","x":0,"y":0,"w":8,"h":8,"v":2,"widget_type":"chart",
            "view":"ext:thecrew/scene","options":{"sceneId":"scene:ahu-1"}}]
}' >/dev/null

echo "-> save 'Scene Builder' dashboard (EMPTY — the palette e2e adds the Scene tile through the UI)"
# An empty editable dashboard so the widget e2e can DRIVE the restored builder palette (finding 7):
# Add panel -> pick 'thecrew · Scene' -> pick the AHU-1 scene -> Save. Idempotent (dashboard.save UPSERT).
call dashboard.save '{
  "id":"scene-build","title":"Scene Builder","now":1,"cells":[]
}' >/dev/null

echo "-> seeded: scene:ahu-1 + ahu1.* series + 'Graphics Scene' + empty 'Scene Builder' dashboards"
call assets.list_docs '{}' | jq -c '.docs'
