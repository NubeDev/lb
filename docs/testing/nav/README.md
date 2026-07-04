---
name: e2e-nav
description: >
  Use when asked to end-to-end test NAV (the persisted, ordered menu NavRail renders). Drive
  the live node's `/navs` CRUD + the workspace-default pointer + the per-user pref + the
  composite `/nav/resolve` lens, against a real node. Assumes suites are green; does NOT
  re-run them.
---

# E2e nav runbook — prove the nav menu works as designed

Status: scope (the standard). Design intent: [`../../scope/`](../../scope/) (nav scope).
Checklist: [`../README.md`](../README.md#what-to-check--the-functional-dimensions).
Policy: [`../../scope/testing/testing-scope.md`](../../scope/testing/testing-scope.md).

**This is real-world verification, not the test suite.** `NavAdmin.gateway.test.tsx` and
`NavRail.test.tsx` are the scope/session's job and assumed green — this runbook does **not**
re-run them. Its job is to **drive the live gateway** through a real nav lifecycle.

A nav is a persisted, ordered menu of items, each linking to a core **surface**, a specific
**dashboard**, an opaque **ext** page, or a dynamic **tag-group**. The nav is a **LENS over
existing access — it grants nothing**: `nav.resolve` returns the caller's effective menu,
already tag-expanded and cap-stripped, and every page verb is re-checked on click regardless.

---

## Step 0. Read the design first

- The nav scope: the four entry kinds (`surface` / `dashboard` / `ext` / `tag-group`, plus
  `group` and `template-group`), the workspace-default pointer, the per-user pick, and
  `nav.resolve` as a pure cap-stripped lens.
- `ui/src/lib/nav/nav.api.ts` + `nav.types.ts` — the drivable surface and the **exact item
  shape** (`{kind, label, surface?, dashboard?, ext?, facets?, items?}`).

> **Item shape gotcha:** a nav item's `kind` is `surface` / `dashboard` / `ext` /
> `tag-group` / `template-group` / `group` — **not** `page`. Posting `{"kind":"page"}` is a
> `400 unknown nav item kind: page`. Use `{"kind":"surface","surface":"channels"}`.

## Step 1. Stand up the running node + token

```bash
make build-wasm && make dev
BASE=http://127.0.0.1:8080
TOKEN=$(curl -s -X POST $BASE/login -H 'content-type: application/json' \
  -d '{"user":"ada","workspace":"acme"}' | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')
A="authorization: Bearer $TOKEN"; C="content-type: application/json"
```

## Step 2. The checklist — driven against the live node

### CRUD — the nav round-trips

```bash
curl -s -X POST $BASE/navs -H "$A" -H "$C" \
  -d '{"id":"e2e-nav","title":"E2E Nav","items":[{"kind":"surface","surface":"channels","label":"Chat"}]}'  # create
curl -s $BASE/navs        -H "$A"    # list
curl -s $BASE/navs/e2e-nav -H "$A"   # get
curl -s -X DELETE $BASE/navs/e2e-nav -H "$A" -o /dev/null -w "%{http_code}\n"   # 204
```

**Observed** (2026-07-04, `acme`): create → `{owner:"user:ada", visibility:"private"}` with
the item normalized to `{kind:"surface", surface:"channels", label:"Chat"}`.

### Functional — the resolve lens + default pointer + per-user pref

```bash
# workspace default → resolve reflects it
curl -s -X POST $BASE/nav/default -H "$A" -H "$C" -d '{"id":"e2e-nav"}' -w "%{http_code}\n"   # 204
curl -s $BASE/nav/resolve -H "$A"    # source:"workspace-default", items cap-stripped
# per-user pick (keyed to the token sub — can't curate another user's pick)
curl -s -X POST $BASE/nav/pref -H "$A" -H "$C" -d '{"id":"e2e-nav"}'   # {"active":"e2e-nav",...}
curl -s $BASE/nav/pref -H "$A"
```

**Observed** (2026-07-04): before any default, `resolve` → `{source:"fallback", items:[]}`;
after `nav/default`, `resolve` → `{source:"workspace-default", nav_id:"e2e-nav", items:[{
kind:"surface", surface:"channels", label:"Chat"}]}`. The resolve output is **cap-stripped**
— an item the caller can't reach is dropped, proving the lens grants nothing.

### Permissions & Access

```bash
curl -s -X POST $BASE/navs -H "$C" -d '{"id":"x","title":"x","items":[]}' -o /dev/null -w "%{http_code}\n"  # NO token → 401
```

A per-verb capability deny (a token holding only `mcp:nav.resolve:call`) is exercised in
`NavAdmin.gateway.test.tsx` via `signInWithCaps("user:ben", ws, [CAP.navResolve])`.
**Access**: a `globex` token's `/navs` and `/nav/resolve` never surface `acme`'s nav — the
default pointer and the pref are both workspace-keyed.

## Step 3–5. What you found / findings / done

Green? Record the output in the session doc. A wrong result → file a
`../../debugging/frontend/…` entry + regression test; not written up here.

Observed green run: [`../../sessions/testing/dashboard-chart-nav-system-session.md`](../../sessions/testing/dashboard-chart-nav-system-session.md).
