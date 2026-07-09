---
name: e2e-charts
description: >
  Use when asked to end-to-end test CHARTS / PANELS backed by the node's own series store
  (the internal telemetry feed, NOT an external datasource). Round-trip a panel definition,
  seed a real sample, and read it back through the series feed a chart renders. For charts
  over an external datasource (the Docker-free SQLite demo, or a real TimescaleDB), use
  `../datasources/` instead. Assumes suites are green.
---

# E2e charts runbook — prove a panel + its series feed work as designed

Status: scope (the standard). Design intent: [`../../scope/`](../../scope/) (library-panels
scope + data-console scope). Checklist:
[`../README.md`](../README.md#what-to-check--the-functional-dimensions).
Policy: [`../../scope/testing/testing-scope.md`](../../scope/testing/testing-scope.md).

**This is real-world verification, not the test suite.** `PanelPage.gateway.test.tsx` is the
scope/session's job and assumed green — this runbook does **not** re-run it. Its job is to
**drive the live node**: save a real panel, seed a real sample through the ingest write path,
and read it back through the exact feed a chart widget renders.

> **Which chart runbook?** This one covers a chart reading the node's **own** series store
> (in-process, no external DB). A chart reading an **external** datasource has a hard seed
> prerequisite — use [`../datasources/README.md`](../datasources/README.md) for that (its
> charts are blank until you seed + register a source; the default Docker-free path is
> `make seed-demo-sqlite`).

A panel is the reusable, non-layout half of a v3 dashboard cell: a `spec` with `sources[]`
that re-check under the **viewer's** caps at render (a lens, never a grant). Owner + ws come
from the token (§7); visibility is set via `/share`.

---

## Step 0. Read the design first

- The library-panels scope (panel = reusable panel definition; `sources[]` re-checked per
  render) + the data-console scope (`ingest.*` / `series.*` verbs).
- `ui/src/lib/ipc/http.ts` — `panel_save` → `POST /panels`, `series_read` →
  `GET /series/{s}/samples`, `ingest_write` → `POST /ingest`.

## Step 1. Stand up the running node + token

```bash
make build-wasm && make dev
BASE=http://127.0.0.1:8080
TOKEN=$(curl -s -X POST $BASE/login -H 'content-type: application/json' \
  -d '{"user":"ada","workspace":"acme"}' | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')
A="authorization: Bearer $TOKEN"; C="content-type: application/json"
```

## Step 2. The checklist — driven against the live node

### CRUD — the panel definition round-trips

```bash
SPEC='{"kind":"timeseries","sources":[{"tool":"series_read","args":{"series":"e2e.temp"}}],"options":{"viz":"line"}}'
curl -s -X POST $BASE/panels -H "$A" -H "$C" -d "{\"id\":\"e2e-chart\",\"title\":\"E2E Chart\",\"spec\":$SPEC}"  # create
curl -s $BASE/panels/e2e-chart       -H "$A"    # read back — sources[] preserved
curl -s $BASE/panels                 -H "$A"    # list
curl -s $BASE/panels/e2e-chart/usage -H "$A"    # which dashboards reference it (→ [])
curl -s -X DELETE $BASE/panels/e2e-chart -H "$A" -o /dev/null -w "%{http_code}\n"   # 204 (throwaway)
# LEAVE a chart in place, bound to the seeded series below, for the user to inspect:
curl -s -X POST $BASE/panels -H "$A" -H "$C" -d "{\"id\":\"keep-chart\",\"title\":\"E2E — leave for inspection\",\"spec\":$SPEC}"
```

**Observed** (2026-07-04, `acme`): create → `{owner:"user:ada", visibility:"private"}` with
`sources[0].tool:"series_read"` preserved; usage → `[]`; delete → `204`.

> Delete is proven on the throwaway `e2e-chart`; **`keep-chart` is left in place**, bound to
> the `e2e.temp` series seeded in the functional step below — so the user can open it and see
> a chart drawing real data (README "Leave it inspectable").

### Functional — the chart actually reads data

The point of a chart is that it renders a real feed. Seed a real sample through the ingest
write path, then read it back through the series feed the panel's `series_read` source uses:

```bash
# NOTE the exact wire shape: producer is overwritten by the token (§7) but the field must be
# present; qos is kebab-case ("best-effort" / "must-deliver").
S='{"series":"e2e.temp","producer":"","ts":1783138000,"seq":1,"payload":21.5,"labels":{"room":"lab"},"qos":"best-effort"}'
curl -s -X POST $BASE/ingest -H "$A" -H "$C" -d "{\"samples\":[$S]}"   # → {"accepted":1,"committed":1}
curl -s "$BASE/series?prefix=e2e"           -H "$A"   # series now listed
curl -s "$BASE/series/e2e.temp/latest"      -H "$A"   # latest sample
curl -s "$BASE/series/e2e.temp/samples"     -H "$A"   # the chart's feed
```

**Observed** (2026-07-04): `accepted:1, committed:1`; `series` → `["e2e.temp"]`; the read
returns the sample with `producer:"user:ada"` (stamped from the token, un-spoofable — a
client-supplied producer is ignored). That round-trip **is** the chart working: a widget
bound to `series_read{series:"e2e.temp"}` now has a point to draw.

> The write route **drains the workspace** before returning, so the just-written sample is
> visible to the very next read (no polling gap). Series are append-only telemetry — there
> is no per-series delete; the seeded `e2e.temp` is harmless dev data.

### Permissions & Access

```bash
curl -s $BASE/panels -o /dev/null -w "%{http_code}\n"   # NO token → 401
```

A per-verb capability deny (missing `mcp:panel.save:call` / `mcp:ingest.write:call`) is
proven server-side in the Rust tests + `signInWithCaps` suite. **Access**: seed a series in
`acme`, sign in to `globex`, confirm `globex`'s `series?prefix=e2e` is empty — the workspace
wall gates the series store too.

## Step 3–5. What you found / findings / done

Green? Record the output in the session doc, **leave `keep-chart` + the `e2e.temp` series in
place**, and hand the user the page: "open `keep-chart` at http://127.0.0.1:8080 — it's
drawing the seeded sample so you can confirm; I only deleted the throwaway panel" (README
"Leave it inspectable"). A wrong result → file a `../../debugging/frontend/…` entry +
regression test; not written up here.

Observed green run: [`../../sessions/testing/dashboard-chart-nav-system-session.md`](../../sessions/testing/dashboard-chart-nav-system-session.md).
