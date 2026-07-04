---
name: e2e-system
description: >
  Use when asked to end-to-end test the SYSTEM MAP (the read-only admin topology/status
  console). Drive the live node's `/system/*` routes and confirm the per-subsystem grid,
  topology, and tool table read as designed — admin-gated, workspace-scoped, read-only.
  Assumes the vitest / cargo suites are already green; this does NOT re-run them.
---

# E2e system runbook — prove the system map works as designed

Status: scope (the standard). Design intent: [`../../scope/system-map/`](../../scope/) (system-map scope).
Checklist: [`../README.md`](../README.md#what-to-check--the-functional-dimensions).
Policy: [`../../scope/testing/testing-scope.md`](../../scope/testing/testing-scope.md).

**This is real-world verification, not the test suite.** The `SystemView.gateway.test.tsx`
suite and the Rust host tests are the scope/session's job and assumed green — this runbook
does **not** re-run them. Its job is to **drive the live node and look at the result**.

The system map is a **read-only** lens: a per-subsystem status grid, a react-flow topology
graph, and the workspace tool table. It is **admin-gated** (`mcp:system.*:call`) and
**workspace-first** — every read is scoped to the token's workspace, never the request body.

---

## Step 0. Read the design first

- The system-map scope: the four reads (`overview`, `topology`, `subsystem/{id}`, `tools`,
  `acp`), each admin-gated, each workspace-scoped, **no write commands by design**.
- `ui/src/lib/system/system.api.ts` — the drivable surface (`system_overview` → `GET
  /system/overview`, etc.).

## Step 1. Stand up the running node

```bash
make build-wasm && make dev      # gateway on 127.0.0.1:8080
```

Mint a session token (the workspace + principal ride the token, never the body):

```bash
BASE=http://127.0.0.1:8080
TOKEN=$(curl -s -X POST $BASE/login -H 'content-type: application/json' \
  -d '{"user":"ada","workspace":"acme"}' | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')
A="authorization: Bearer $TOKEN"
```

## Step 2. The checklist — driven against the live node

The system map owns no entity (read-only), so the CRUD dimension collapses to **read**; the
mandatory pair is **permissions** and **access**, plus the **functional** reads.

### Read (functional) — every lens returns a workspace-scoped snapshot

```bash
curl -s $BASE/system/overview  -H "$A"   # per-subsystem status grid
curl -s $BASE/system/topology  -H "$A"   # nodes + wiring edges (react-flow)
curl -s $BASE/system/tools     -H "$A"   # workspace tool table
curl -s $BASE/system/acp       -H "$A"   # ACP adapter facts
```

Assert: each returns `200` with `"ws"` = the token's workspace and a non-empty
`services` / `nodes` / `tools` list. **Observed** (2026-07-04, `acme`): `overview` →
`role:"solo"`, subsystems `gateway|bus|mcp|extensions` all `health:"ok"`; `topology` →
same nodes with wiring; `tools` → the host tool table (`agent.decide`, …).

### Permissions — the negative path is the point

```bash
curl -s $BASE/system/overview                    -o /dev/null -w "%{http_code}\n"  # NO token  → 401
curl -s $BASE/system/overview -H "authorization: Bearer not.a.jwt" \
                                                  -o /dev/null -w "%{http_code}\n"  # garbage   → 401
curl -s $BASE/system/subsystem/nonexistent -H "$A" -o /dev/null -w "%{http_code}\n" # unknown id → 403 (opaque deny)
```

An unknown subsystem id is `403`-opaque — a denial looks the same as a miss, so a caller
can't probe the topology. A **capability** deny (a valid token *without* the admin cap)
is proven server-side in the Rust host tests and in the `signInWithCaps` gateway suite —
dev-login carries `mcp:system.*:call`, so a deny case passes fewer caps.

### Access — the workspace wall

The `"ws"` field on every response equals the token's workspace. Sign in to a second
workspace (`--globex`) and confirm its `overview`/`topology` reflect *its* subsystems only —
never `acme`'s. The wall is checked before caps.

## Step 3–5. What you found / findings / done

Green? Record the command output in the session doc (green is a claim that must be shown).
A wrong result → file a `../../debugging/frontend/…` (or `.../system/…`) entry + a
regression test; **do not** write the failure up here. Known pre-existing red:
`SystemView.gateway` (`bus peers list` label) — memory: preexisting-failing-tests; don't
chase it as your regression.

Observed green run: [`../../sessions/testing/dashboard-chart-nav-system-session.md`](../../sessions/testing/dashboard-chart-nav-system-session.md).
