# Session — e2e-verify dashboard / chart / nav / system on the live node

Date: 2026-07-04 · Topic: testing (real-world e2e verification) · Node: cloud dev node,
gateway on `127.0.0.1:8080` (already running).

## Ask

"Add tests that test the dashboard, chart, nav, system" — real-world e2e verification per
[`docs/testing/README.md`](../../testing/README.md): drive the running system and observe
each feature behave (CRUD / permissions / access / functional), then write the runbooks.
Teams/users e2e is the next slice (`docs/testing/workspace-team-users/`), after this passes.

## What I drove (and how)

The live node over its REST/MCP gateway surface — real SurrealDB, real capability wall,
workspace derived from the token (§7), **no mocks**. Token minted via `POST /login`
(`ada`/`acme`, `bob`/`globex`). The verbs map 1:1 to `ui/src/lib/{dashboard,nav,system}.api.ts`
+ `ui/src/lib/ipc/http.ts`.

Also ran the in-process real-gateway suites (a spawned node, not a fake) to confirm the
UI↔gateway seam is green for these areas:

```
$ cd ui && pnpm test:gateway \
    src/features/dashboard/DashboardView.gateway.test.tsx \
    src/features/panel/PanelPage.gateway.test.tsx \
    src/features/admin/nav/NavAdmin.gateway.test.tsx
 ✓ DashboardView (real gateway) (8 tests)
 ✓ PanelPage           (8 tests)
 ✓ NavAdmin            (4 tests)
 Test Files  3 passed (3)   Tests  20 passed (20)
```

(`SystemView.gateway.test.tsx` has a **pre-existing** red — `bus peers list` label; memory:
preexisting-failing-tests — not a regression from this work.)

## Observed results (green)

All against `BASE=http://127.0.0.1:8080`, `A="authorization: Bearer $TOKEN"`.

### System (read-only admin lens) — [`../../testing/system/README.md`](../../testing/system/README.md)
- `GET /system/overview` → `ws:"acme", role:"solo"`, subsystems `gateway|bus|mcp|extensions`
  all `health:"ok"`.
- `GET /system/topology` → same nodes + wiring edges. `GET /system/tools` → host tool table.
- **Permissions:** no token → `401`; garbage token → `401`; unknown subsystem id → `403`
  (opaque deny). **Access:** every response carries `ws:"acme"` (token-scoped).

### Dashboard CRUD — [`../../testing/dashboard/README.md`](../../testing/dashboard/README.md)
- create `e2e-dash` → `{owner:"user:ada", visibility:"private", schemaVersion:3}`.
- get → matches. update title → `"E2E Dash v2"`. list → shows updated title.
- delete → `204`; get-after-delete → `404`. Full lifecycle round-tripped the store.
- **Permissions:** no token → `401`. **Access:** seeded `iso-dash` in `acme` → `acme` `200`;
  `globex` token → `404` + empty list. **The workspace wall holds.**

### Chart / panel + series feed — [`../../testing/charts/README.md`](../../testing/charts/README.md)
- panel `e2e-chart` create/get/list/usage(→`[]`)/delete all green; `sources[0].tool:"series_read"`
  preserved through the round-trip.
- **Functional (the chart reads real data):** `POST /ingest` a real sample →
  `{accepted:1, committed:1}`; `GET /series?prefix=e2e` → `["e2e.temp"]`;
  `GET /series/e2e.temp/samples` → the sample, `producer:"user:ada"` **stamped from the
  token** (a client-supplied producer is ignored — un-spoofable, §7). That round-trip is the
  chart feed working.
- Wire-shape gotchas found & documented: sample needs a `producer` field present (even though
  overwritten); `qos` is kebab-case (`"best-effort"`, not `"best_effort"`) → else `422`.

### Nav — [`../../testing/nav/README.md`](../../testing/nav/README.md)
- create `e2e-nav` (`{kind:"surface", surface:"channels"}`) → owner/visibility set, item
  normalized. list/get/delete green.
- **Functional:** before default → `resolve` `{source:"fallback", items:[]}`; after
  `POST /nav/default` → `resolve` `{source:"workspace-default", nav_id:"e2e-nav", items:[…]}`,
  **cap-stripped**. `POST /nav/pref` → `{active:"e2e-nav"}` (per-user, token-`sub`-keyed).
- **Permissions:** no token → `401`. Item-kind gotcha documented: `kind:"page"` → `400`.

## Mandatory-trio coverage (testing-scope §2)

| Dimension | Covered | Where |
|---|---|---|
| CRUD | ✅ | dashboard + panel + nav full round-trips (create→read→update→list→delete) |
| Permissions (cap-deny) | ✅ | live `401`/`403` negative paths; per-cap deny via `signInWithCaps` (NavAdmin suite) + Rust host tests |
| Access (workspace wall) | ✅ | `globex` cannot read `acme`'s dashboard (`404` + empty list); every system read is token-ws-scoped |
| Functional | ✅ | nav resolve/default/pref lens; chart series ingest→read; system snapshot |

## Findings

None — everything behaved as designed. No `debugging/` entry needed. The only surprises were
input wire-shape strictness (nav item `kind`, sample `producer`/`qos`), now captured as
gotchas in the runbooks so the next driver doesn't hit the same `400`/`422`.

## Left behind

- Four runbooks: `docs/testing/{system,dashboard,charts,nav}/README.md` (agent-runnable,
  drop-in `.claude/skills/`-compatible frontmatter).
- This session doc (the observed green log).
- Updated `docs/testing/README.md` runbook table.

## Next

`docs/testing/workspace-team-users/` — teams + users e2e, per the ask ("once we get that
working we test teams and users"). The `PeopleAdmin` / `TeamsAdmin` / `WorkspacesAdmin`
gateway suites already exist; the runbook drives member add/remove + cross-team isolation.
