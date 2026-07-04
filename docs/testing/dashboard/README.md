---
name: e2e-dashboard
description: >
  Use when asked to end-to-end test DASHBOARDS. Drive the live node's `/dashboards` routes
  through a full create → read → update → list → delete round-trip, prove the workspace wall
  and the auth deny, against a real node. Assumes the vitest / cargo suites are already
  green; this does NOT re-run them.
---

# E2e dashboard runbook — prove dashboard CRUD works as designed

Status: scope (the standard). Design intent: [`../../scope/`](../../scope/) (dashboard scope).
Checklist: [`../README.md`](../README.md#what-to-check--the-functional-dimensions).
Policy: [`../../scope/testing/testing-scope.md`](../../scope/testing/testing-scope.md).

**This is real-world verification, not the test suite.** `DashboardView.gateway.test.tsx`
is the scope/session's job and assumed green — this runbook does **not** re-run it. Its job
is to **drive the live gateway and round-trip a real dashboard record through the store**.

A dashboard is an owned, workspace-scoped asset: the owner + workspace come from the token
(§7); visibility (`private` / `team` / `workspace`) is set via `/share`, never on save. The
gateway re-checks all three gates (workspace → cap → membership/visibility) server-side.

---

## Step 0. Read the design first

- The dashboard scope: the asset shape (`id`, `title`, `cells[]`, `variables[]`,
  `schemaVersion:3`, `visibility`, `owner`), the CRUD verbs, the share tiers.
- `ui/src/lib/dashboard/dashboard.api.ts` — the drivable surface (`dashboard_save` →
  `POST /dashboards`, `dashboard_get` → `GET /dashboards/{id}`, …).

## Step 1. Stand up the running node + token

```bash
make build-wasm && make dev
BASE=http://127.0.0.1:8080
TOKEN=$(curl -s -X POST $BASE/login -H 'content-type: application/json' \
  -d '{"user":"ada","workspace":"acme"}' | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')
A="authorization: Bearer $TOKEN"; C="content-type: application/json"
```

## Step 2. The checklist — driven against the live node

### CRUD — the full lifecycle, round-tripped through the real store

```bash
# CREATE (save is upsert; owner + ws come from the token, not the body)
curl -s -X POST $BASE/dashboards -H "$A" -H "$C" \
  -d '{"id":"e2e-dash","title":"E2E Dash","cells":[],"variables":[]}'
# READ back — assert the shape matches scope
curl -s $BASE/dashboards/e2e-dash -H "$A"
# UPDATE one field, re-read
curl -s -X POST $BASE/dashboards -H "$A" -H "$C" \
  -d '{"id":"e2e-dash","title":"E2E Dash v2","cells":[],"variables":[]}'
# LIST — new title appears
curl -s $BASE/dashboards -H "$A"
# DELETE, then assert it's gone
curl -s -X DELETE $BASE/dashboards/e2e-dash -H "$A" -o /dev/null -w "%{http_code}\n"   # 204
curl -s $BASE/dashboards/e2e-dash -H "$A"      -o /dev/null -w "%{http_code}\n"        # 404
```

A create you never read back proves nothing — the read-back (and the survives-refresh) is
what proves it round-tripped the store, not local state. **Observed** (2026-07-04, `acme`):
create → `{owner:"user:ada", visibility:"private", schemaVersion:3}`; update → title
changed; delete → `204`; get-after-delete → `404`.

### Permissions — the negative path

```bash
curl -s $BASE/dashboards -o /dev/null -w "%{http_code}\n"   # NO token → 401
```

A per-verb **capability** deny (valid token missing `mcp:dashboard.save:call`) is proven
server-side in the Rust tests and via `signInWithCaps` in the gateway suite — dev-login
carries the save cap, so a deny case passes fewer caps.

### Access — the workspace wall (mandatory)

```bash
# seed in acme
curl -s -X POST $BASE/dashboards -H "$A" -H "$C" \
  -d '{"id":"iso-dash","title":"secret","cells":[],"variables":[]}' -o /dev/null
# a globex token must NOT see it
TB=$(curl -s -X POST $BASE/login -H "$C" -d '{"user":"bob","workspace":"globex"}' \
  | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')
curl -s $BASE/dashboards/iso-dash -H "authorization: Bearer $TB" -o /dev/null -w "%{http_code}\n"  # 404
curl -s $BASE/dashboards         -H "authorization: Bearer $TB"                                    # []
curl -s -X DELETE $BASE/dashboards/iso-dash -H "$A" -o /dev/null   # cleanup
```

**Observed** (2026-07-04): `acme` sees `iso-dash` (`200`); `globex` gets `404` and an empty
list. The wall is checked **before** caps.

## Step 3–5. What you found / findings / done

Green? Record the output in the session doc. A wrong result → file a
`../../debugging/frontend/…` entry + a regression `*.gateway.test.tsx`; do **not** write the
failure up here.

Observed green run: [`../../sessions/testing/dashboard-chart-nav-system-session.md`](../../sessions/testing/dashboard-chart-nav-system-session.md).
