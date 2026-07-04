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
# prove DELETE on a THROWAWAY (not e2e-nav — the functional step + the user inspect it)
curl -s -X POST   $BASE/navs -H "$A" -H "$C" -d '{"id":"tmp-nav","title":"tmp","items":[]}' -o /dev/null
curl -s -X DELETE $BASE/navs/tmp-nav -H "$A" -o /dev/null -w "%{http_code}\n"   # 204
```

**Observed** (2026-07-04, `acme`): create → `{owner:"user:ada", visibility:"private"}` with
the item normalized to `{kind:"surface", surface:"channels", label:"Chat"}`.

> `e2e-nav` is **kept** — the functional step wires it as the workspace default, and it's
> what the user opens to confirm. Delete is proven on the throwaway `tmp-nav` (README
> "Leave it inspectable").

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

**Access**: a `globex` token's `/navs` and `/nav/resolve` never surface `acme`'s nav — the
default pointer and the pref are both workspace-keyed.

### Read-only (viewer) — nav EDITING is capability-gated, resolving is not

Editing the nav is **not** a member right. Authoring rides the admin-ish write caps —
`mcp:nav.save:call` (create/update = `POST /navs`, and it *also* gates the workspace-default
pointer `POST /nav/default` — see [`nav.rs`](../../../rust/role/gateway/src/routes/nav.rs)
`set_default_nav`, "Gated `nav.save` (admin-ish)"), `mcp:nav.delete:call` (`DELETE /navs/{id}`),
and `mcp:nav.share:call`. A **viewer** (read-only) holds only the reads — `mcp:nav.resolve:call`
(member-level,
the lens `NavRail` renders) and `nav.list`/`nav.get`. A viewer can **read** their menu but
**cannot author, delete, share, or set the default**; the gateway returns **`403`** on each
write. In the shell this is the same posture as the [dashboard viewer
mode](../../public/frontend/dashboard.md#viewer-mode--editing-the-surface-is-admin-only-shipped-2026-07-04): the
`NavAdmin` builder tab only renders for a token holding `nav.save` (`hasCap(caps, CAP.navSave)`
→ otherwise "You need the nav authoring capability" placeholder), and the gateway re-checks
regardless — the UI gate is convenience, the `403` is the wall.

**Why you can't fully drive this on the live `make dev` node:** dev `/login` always mints the
**full dev claim set** (every login on the dev node resolves to workspace-admin — there is no
`caps` override on `/login`), so a live curl always carries `nav.save`. A genuine read-only
token needs a **narrowed cap set**, which is minted by the test gateway's `/_seed/session`
route (`test_gateway_seed.rs` → `signInWithCaps`), not the live node. So the read-only proof is
a **gateway test**, and this runbook points at it rather than faking a narrow live token.

The drivable read-only assertion — a viewer resolves but every write `403`s — lives in
[`NavAdmin.gateway.test.tsx`](../../../ui/src/features/admin/nav/NavAdmin.gateway.test.tsx),
which signs in a viewer with **only** the resolve cap:

```ts
// Ben logs in read-only — nav.resolve ONLY, NO nav.save/delete/share/default.
await signInWithCaps("user:ben", ws, [CAP.navResolve]);
const resolved = await resolveNav();            // ✓ 200 — the lens still renders his menu
// and every authoring verb is refused server-side (the wall, not the UI):
await expect(saveNav("x", "X", [])).rejects.toThrow();       // 403 — nav.save denied
await expect(deleteNav("x")).rejects.toThrow();              // 403 — nav.delete denied
await expect(setDefaultNav("x")).rejects.toThrow();          // 403 — needs nav.save (gates /nav/default)
```

The file already proves the **resolve half** (`signInWithCaps("user:ben", ws, [CAP.navResolve])`
→ `resolve` returns the `workspace-default` menu, cap-stripped). Extend it with the three
`rejects.toThrow()` write-deny lines above so the read-only posture is asserted in **both**
directions — reads through, writes refused — mirroring the dashboard viewer-mode gateway test
(VIEWER + VIEWER-DENY). That is the "nav is read-only / not editable" test.

> If you must sanity-check on the live node without a narrow token, you can only prove the
> **positive** side there: any dev token `GET /nav/resolve` returns a cap-stripped menu and
> every page verb re-checks on click. The **negative** side (a viewer's `POST /navs` → `403`)
> is the gateway test's job, because the live node can't mint the viewer token.

## Step 3–5. What you found / findings / done

Green? Record the output in the session doc, **leave `e2e-nav` in place as the workspace
default**, and hand the user the page: "open the app at http://127.0.0.1:8080 — the nav is
set so you can confirm; I only removed the throwaway `tmp-nav`" (README "Leave it
inspectable"). A wrong result → file a `../../debugging/frontend/…` entry + regression test;
not written up here.

Observed green run: [`../../sessions/testing/dashboard-chart-nav-system-session.md`](../../sessions/testing/dashboard-chart-nav-system-session.md).
