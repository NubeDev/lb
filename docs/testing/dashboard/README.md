---
name: e2e-dashboard
description: >
  Use when asked to end-to-end test DASHBOARDS, WIDGETS/PANELS, or dashboard VARIABLES. Drive the
  live node's `/dashboards` + `/panels` routes + the `dashboard.*`/`panel.*` MCP verbs: full CRUD,
  library-panel reuse (ref cells), and the Grafana-style variable system — definition round-trip,
  the required-var access gate (`dashboard.access_check`), and URL-selection/interpolation. Prove
  the workspace wall + the cap deny against a real node + the seeded `demo-buildings` source. Assumes
  the vitest / cargo suites are already green; this does NOT re-run them.
---

# E2e dashboard runbook — prove dashboards, panels + variables work as designed

Status: scope (the standard). Design intent: [`../../scope/frontend/dashboard/`](../../scope/frontend/dashboard/)
(dashboard + [`library-panels-scope.md`](../../scope/frontend/dashboard/library-panels-scope.md) +
[`widget-config-vars-scope.md`](../../scope/frontend/dashboard/widget-config-vars-scope.md) +
[`reusable-pages-scope.md`](../../scope/frontend/dashboard/reusable-pages-scope.md)) and
[`../../scope/frontend/query-builder/`](../../scope/frontend/query-builder/).
Drivable surface: [`../../skills/dashboard-mcp/SKILL.md`](../../skills/dashboard-mcp/SKILL.md),
[`../../skills/panels/SKILL.md`](../../skills/panels/SKILL.md).
Checklist: [`../README.md`](../README.md#what-to-check--the-functional-dimensions).
Policy: [`../../scope/testing/testing-scope.md`](../../scope/testing/testing-scope.md).

**This is real-world verification, not the test suite.** `DashboardView.gateway.test.tsx` +
`PanelPage.gateway.test.tsx` + the `vars/*.test.ts` unit suite are the scope/session's job and assumed
green — this runbook does **not** re-run them. Its job is to **drive the live gateway and round-trip
real records through the store**: a dashboard, a reusable panel, and a working variable.

A dashboard is an owned, workspace-scoped asset: owner + workspace come from the token (§7);
visibility (`private`/`team`/`workspace`) is set via `/share`, never on save. Variables are
**definitions on the record** (`dashboard.save` round-trips them); the **selection lives in the URL**
(`?var-<name>=`, Grafana parity), never on the record.

---

## Step 0 — stand up the node AND seed the datasource

The CRUD/panel checks need only a running node. The **variable** checks bind a real query-variable to
the seeded `demo-buildings` SQLite source (a `building` dropdown over the 8 seeded sites), so seed it
first (Docker-free — see [`../datasources/README.md`](../datasources/README.md) Step 0):

```bash
make build-wasm && make dev          # boot the node (root on 8080)
make seed-demo-sqlite                # generates buildings.db + datasource.add {kind:sqlite} (for the var checks)

BASE=http://127.0.0.1:8080
TOKEN=$(curl -s -X POST $BASE/login -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
A="authorization: Bearer $TOKEN"; C="content-type: application/json"
```

---

## Step 1 — read the design (what is "correct"?)

- **[`../../skills/dashboard-mcp/SKILL.md`](../../skills/dashboard-mcp/SKILL.md)** — the dashboard verbs
  (`dashboard.save|get|list|delete|share`), the cell model, `variables[]`. **REST fills `now`; `/mcp/call`
  requires it.** `save` is an idempotent UPSERT (read-modify-write to avoid clobbering).
- **[`../../skills/panels/SKILL.md`](../../skills/panels/SKILL.md)** — library panels (`panel.*`), ref
  cells, delete-safety, the standalone page, binding a panel to a saved rule.
- **[`widget-config-vars-scope.md`](../../scope/frontend/dashboard/widget-config-vars-scope.md)** — the
  variable model (6 types), the three syntaxes (`$var`/`${var}`/`[[var]]`), format hints, the built-ins
  (`$__from`/`$__to`/`$__interval`/`${__user.login}`/`${__workspace}`), URL sync, refresh, the JSON
  payload builder. **Selection is URL, definitions are the record; identity built-ins are token-resolved,
  never cell-set.**
- **[`reusable-pages-scope.md`](../../scope/frontend/dashboard/reusable-pages-scope.md)** — a `required`
  variable turns a dashboard into a **template**: unbound (no URL value, no default) → the honest
  "select a `<label>`" gate instead of firing a `$name`-literal query.

---

## Step 2 — the checklist (drive it, observe it works as designed)

### 2.1 CRUD — the dashboard lifecycle round-trips

```bash
# CREATE (save is upsert; owner + ws come from the token, not the body)
curl -s -X POST $BASE/dashboards -H "$A" -H "$C" \
  -d '{"id":"e2e-dash","title":"E2E Dash","cells":[],"variables":[]}'
curl -s $BASE/dashboards/e2e-dash -H "$A"                       # READ back — {owner, visibility, schemaVersion:3}
curl -s -X POST $BASE/dashboards -H "$A" -H "$C" \
  -d '{"id":"e2e-dash","title":"E2E Dash v2","cells":[],"variables":[]}'   # UPDATE title
curl -s $BASE/dashboards -H "$A"                                # LIST — new title appears
curl -s -X DELETE $BASE/dashboards/e2e-dash -H "$A" -o /dev/null -w "%{http_code}\n"   # 204 (throwaway)
curl -s $BASE/dashboards/e2e-dash -H "$A"      -o /dev/null -w "%{http_code}\n"        # 404
```

A create you never read back proves nothing — the read-back is what proves it round-tripped the store,
not local state. **Observed shape:** `{owner:"user:ada", visibility:"private", schemaVersion:3}`.

### 2.2 Panels — a reusable library panel + a ref cell

A **library panel** is the non-layout half of a cell (`panel:{id}`), reused across dashboards. Bind one
to the seeded source and reference it from a dashboard cell:

```bash
# a library panel over the seeded source (federation.query against demo-buildings)
curl -s -X POST $BASE/panels -H "$A" -H "$C" -d '{
  "id":"energy-by-building","title":"Energy by building",
  "spec":{"v":3,"view":"timeseries","widget_type":"chart","title":"Energy by building",
    "sources":[{"refId":"A","tool":"federation.query","args":{"source":"demo-buildings",
      "sql":"SELECT s.name AS building, ROUND(SUM(pr.value),0) AS kwh FROM point_reading pr JOIN point p ON p.id=pr.point_id JOIN meter m ON m.id=p.meter_id JOIN site s ON s.id=m.site_id WHERE p.name='"'"'Energy kWh'"'"' GROUP BY s.name ORDER BY kwh DESC"}}]}}'
curl -s -X POST $BASE/panels/energy-by-building/share -H "$A" -H "$C" -d '{"visibility":"workspace"}'

# a dashboard that REFERENCES it (a ref cell — layout + panelRef, NO spec)
curl -s -X POST $BASE/dashboards -H "$A" -H "$C" -d '{
  "id":"keep-dash","title":"E2E — leave for inspection",
  "cells":[{"i":"c1","x":0,"y":0,"w":8,"h":4,"panelRef":"panel:energy-by-building"}],"variables":[]}'

# the read comes back HYDRATED — the ref cell has the panel's view/sources spliced in host-side
curl -s $BASE/dashboards/keep-dash -H "$A" | jq '.cells[0] | {i, panelRef, view}'
# → {"i":"c1","panelRef":"panel:energy-by-building","view":"timeseries"}

# delete-safety: the panel is in use, so a plain delete REFUSES (returns the usage list)
curl -s $BASE/panels/energy-by-building/usage -H "$A" | jq              # → {"usage":[{"dashboard":"keep-dash",…}]}
curl -s -X DELETE $BASE/panels/energy-by-building -H "$A" -i | head -1   # → 400 (in use); force=true would tombstone
```

**Observe:** the ref cell hydrates the panel's spec at `dashboard.get` (edit the panel once → every
referencing dashboard updates); `panel.delete` **refuses while referenced** (pass `force=true` to
tombstone, and the ref then hydrates to an honest `panelMissing` placeholder — no spec leak).

#### ⚠ Gotcha (found live): a hand-authored cell shows NO SQL in the panel editor

Opening the panel editor (`…/dashboards/<d>/new-panel?cell=<i>`) on a cell authored by raw curl can show
an **empty SQL box** even though the cell queries fine. Two causes, both because the builder writes
fields a raw POST omits — verified live 2026-07-09:

- **A `panelRef` (library-panel) cell is not inline-editable.** The editor shows a "Library panel — used
  on N dashboards" banner, not the Query tab — editing a ref cell means editing the underlying *panel*.
  So a `panelRef` cell will never show inline SQL by design.
- **An inline cell needs the editor's two rehydration fields.** The SQL box reads
  **`cell.options.sql.rawSql`** (`ui/src/lib/panel-kit/cellEditorState.ts` → `state.sql`), and the
  datasource picker reads **`cell.sources[0].datasource`** (`{type:"federation", uid:"datasource:<ws>:<name>"}`).
  A raw POST that sets only `sources[0].args.sql` leaves `options.sql` null and `datasource` null, so the
  builder opens blank. (The `fedSql` fallback in `QueryTab.tsx` recovers the string only when
  `isFederation` resolves — belt-and-suspenders is to set both fields.)

**Author an editor-visible inline federation cell like this** (the shape the builder itself saves):

```bash
SQL="SELECT s.name AS building, ROUND(SUM(pr.value),0) AS kwh FROM point_reading pr JOIN point p ON p.id=pr.point_id JOIN meter m ON m.id=p.meter_id JOIN site s ON s.id=m.site_id WHERE p.name='Energy kWh' GROUP BY s.name ORDER BY kwh DESC"
jq -n --arg sql "$SQL" '{
  id:"keep-dash", title:"E2E — panel (editor-visible)",
  cells:[{ i:"c1", x:0, y:0, w:8, h:4, v:3, widget_type:"chart", view:"table", title:"Energy by building",
    sources:[{refId:"A", tool:"federation.query",
      datasource:{type:"federation", uid:"datasource:acme:demo-buildings"},   # ← the picker reads this
      args:{source:"demo-buildings", sql:$sql}}],
    options:{ sql:{ mode:"code", format:"table", rawSql:$sql } } }],   # ← the SQL box reads this
  variables:[] }' \
| curl -s -X POST $BASE/dashboards -H "$A" -H "$C" -d @- -o /dev/null -w "save: %{http_code}\n"
```

**Observe:** now `…/new-panel?cell=c1` opens with the SQL populated and the `demo-buildings` datasource
selected. This is the difference between a cell that *runs* and a cell that *round-trips through the
editor* — a raw-curl cell does the former; the builder writes both. For a **variable** cell keep the
`${site}` template in **both** `sources[0].args.sql` and `options.sql.rawSql`.

### 2.3 Variables — the Grafana-style variable system (test all three layers)

Variables have **three testable layers**; drive each against the live node.

**(a) A `site` query-variable + a cell that re-points by it — round-tripped, then FIRED.**
Save a dashboard with a **query** variable `site` (its dropdown resolves from the seeded source) and a
cell whose SQL references `${site}`. This is the exact flow driven live on 2026-07-09 (`acme`) — the
outputs below are observed, not illustrative:

```bash
curl -s -X POST $BASE/dashboards -H "$A" -H "$C" -d '{
  "id":"e2e-site-vars","title":"E2E — Site variable",
  "variables":[
    {"name":"site","label":"Site","type":"query","multi":false,"includeAll":false,
     "query":{"tool":"federation.query","args":{"source":"demo-buildings",
       "sql":"SELECT DISTINCT s.name AS site FROM site s ORDER BY site"}}}
  ],
  "cells":[{"i":"c1","x":0,"y":0,"w":12,"h":6,"title":"Energy kWh for ${site}",
    "source":{"tool":"federation.query","args":{"source":"demo-buildings",
      "sql":"SELECT s.name AS site, ROUND(SUM(pr.value),0) AS kwh FROM point_reading pr JOIN point p ON p.id=pr.point_id JOIN meter m ON m.id=p.meter_id JOIN site s ON s.id=m.site_id WHERE s.name = '"'"'${site}'"'"' AND p.name='"'"'Energy kWh'"'"' GROUP BY s.name"}}}]}'
# → {"id":"e2e-site-vars","owner":"user:ada","visibility":"private","schemaVersion":3}

# read back — the site variable definition + the ${site} cell template round-trip through the store
curl -s $BASE/dashboards/e2e-site-vars -H "$A" | jq '{var: .variables[0].name, cell_sql: .cells[0].source.args.sql}'
# → {"var":"site", "cell_sql":"…WHERE s.name = '${site}' AND p.name='Energy kWh'…"}   (the template is stored literally)
```

**⚠ The binding mechanism differs by source kind — this is the gotcha the live run surfaced:**
- **`federation.query`** (external sqlite/postgres) does **NOT** bind a `vars` param map — a
  `{"sql":"… = $site","vars":{"site":"…"}}` returns **empty** (the `$site` reaches the SQLite engine
  literally and matches nothing; verified live). The shell's shared `vars` library **interpolates
  `${site}` into the SQL string** for a federation cell — so the stored template is `s.name = '${site}'`.
- **`store.query`** (SurrealDB) is the opposite: it binds `${var}` as a **`{sql, vars}` parameter**
  (anti-injection), never string-spliced. Use the param form there.

Pick the mechanism by the cell's source kind. (The vars-scope's "no string-spliced SQL" rule is about
`store.query`; federation interpolates.)

```bash
# resolve the dropdown options (what the variable bar shows) — the real federation.query verb
curl -s -X POST $BASE/mcp/call -H "$A" -H "$C" \
  -d '{"tool":"federation.query","args":{"source":"demo-buildings","sql":"SELECT DISTINCT s.name AS site FROM site s ORDER BY site"}}' \
  | jq -c '[.rows[][0]]'
# → ["Airport Terminal T2","Central Mall","Eastfield Warehouse","Lakeside Apartments",
#    "Northside Factory","Riverside Data Center","Southbank Office","Westend Hospital"]   (8 sites, live)

# FIRE the cell as the shell would after ?var-site=Westend Hospital — interpolate ${site}, call the source
T=$(curl -s $BASE/dashboards/e2e-site-vars -H "$A" | jq -r '.cells[0].source.args.sql')
curl -s -X POST $BASE/mcp/call -H "$A" -H "$C" \
  -d "$(jq -n --arg sql "${T//'${site}'/Westend Hospital}" '{tool:"federation.query",args:{source:"demo-buildings",sql:$sql}}')" \
  | jq -c '{columns, rows}'
# → {"columns":["site","kwh"],"rows":[["Westend Hospital",17321]]}   (observed live)
# pick a different site → the panel re-points: Northside Factory → 14153, Central Mall → 13595 (all observed)
```

**Observe:** the `site` variable definition round-trips unchanged (additive `#[serde(default)]` — a
pre-variables dashboard round-trips too); the dropdown resolves 8 real sites; and interpolating a
selected site into the cell template returns that site's real kWh. **Changing the selection re-points
the same panel** — the whole point of a variable.

**(b) The required-var ACCESS GATE — the capability seam (`dashboard.access_check`).**
Mark a variable `required` (a page parameter): the dashboard becomes a **template**, and the access
preflight reports, per dependency, whether a subject can bind it. **`kind` is lowercase `"variable"`**
in the report (the live gotcha — a `select(.kind=="Variable")` filter matches nothing). All three
verdicts below were driven live on 2026-07-09 and the outputs are observed:

```bash
# required QUERY var, caller HOLDS federation.query → "unchecked" (v1 doesn't walk per-value options,
# so it is honestly NOT claimed green — this keeps the overall report from over-promising)
curl -s -X POST $BASE/dashboards -H "$A" -H "$C" -d '{
  "id":"e2e-site-req","title":"required site var",
  "variables":[{"name":"site","label":"Site","type":"query","required":true,
    "query":{"tool":"federation.query","args":{"source":"demo-buildings","sql":"SELECT DISTINCT s.name AS site FROM site s ORDER BY site"}}}],
  "cells":[]}' -o /dev/null
curl -s -X POST $BASE/mcp/call -H "$A" -H "$C" \
  -d '{"tool":"dashboard.access_check","args":{"dashboard":"e2e-site-req","now":1}}' \
  | jq -c '{overall_ok:.ok, var:(.dependencies[]|select(.kind=="variable"))}'
# → {"overall_ok":false,"var":{"dep":"var:site","kind":"variable","ok":false,"unchecked":true,
#    "reason":"resolver cap held; per-value option resolution not validated in v1"}}

# required var with a STATIC default → trivially bindable, no resolver cap needed
curl -s -X POST $BASE/dashboards -H "$A" -H "$C" \
  -d '{"id":"e2e-staticvar","title":"static req var","variables":[{"name":"env","type":"custom","required":true,"custom":["prod","staging"]}],"cells":[]}' -o /dev/null
curl -s -X POST $BASE/mcp/call -H "$A" -H "$C" \
  -d '{"tool":"dashboard.access_check","args":{"dashboard":"e2e-staticvar","now":1}}' \
  | jq -c '{overall_ok:.ok, var:(.dependencies[]|select(.kind=="variable"))}'
# → {"overall_ok":true,"var":{"dep":"var:env","ok":true,"reason":"bindable (static default/options)"}}

# required QUERY var with NO bindable source (empty resolver tool) → a hard gap
curl -s -X POST $BASE/dashboards -H "$A" -H "$C" \
  -d '{"id":"e2e-badvar","title":"bad required var","variables":[{"name":"orphan","type":"query","required":true,"query":{"tool":"","args":{}}}],"cells":[]}' -o /dev/null
curl -s -X POST $BASE/mcp/call -H "$A" -H "$C" \
  -d '{"tool":"dashboard.access_check","args":{"dashboard":"e2e-badvar","now":1}}' \
  | jq -c '{overall_ok:.ok, var:(.dependencies[]|select(.kind=="variable"))}'
# → {"overall_ok":false,"var":{"dep":"var:orphan","ok":false,"unchecked":false,
#    "reason":"required var has no bindable source"}}

# cleanup the three access-gate scaffolds (keep the primary e2e-site-vars dashboard)
for d in e2e-site-req e2e-staticvar e2e-badvar; do curl -s -X DELETE $BASE/dashboards/$d -H "$A" -o /dev/null; done
```

**Observe the seam:** `dashboard.access_check` walks the dependency closure and returns a per-variable
verdict — `ok:true` (static/bindable), `unchecked:true` (query-var, cap held, options not walked in v1),
or `ok:false` (no bindable source). The overall `ok` is the AND, so an `unchecked` hop keeps it honest
(not-all-green). The **hard deny** — a required query-var whose resolver cap the subject *lacks* →
`ok:false, "the subject lacks the required var's resolver cap"` — needs a subject narrowed below
`federation.query`; since live `/login` always mints the full dev claim set, that variant is the
`access_check` cargo test / `DashboardView.gateway.test.tsx` (a `signInWithCaps` token), not a live curl.
The live curl proves the **held**, **static**, and **no-source** paths (all shown above).

> Preflighting **another** subject's reach (`{"subject":"user:bob"}`) is admin-ish — it needs
> `mcp:authz.resolve:call`; a self-preflight (subject == caller) is member-level.

**(c) Selection + interpolation live in the URL/shell (the vitest layer, pointed at the gateway).**
Selection is per-viewer and shareable, so it is **not** on the record — it rides the URL
(`?var-site=Westend%20Hospital`, repeated for multi; `?from=`/`?to=`/`?refresh=`), and the shared `vars` library
(`ui/src/lib/vars/`) interpolates `$var`/`${var}`/`[[var]]` + the built-ins into each cell's
`source.args` before the bridge call. The **identity built-ins** (`${__user.login}`/`${__workspace}`)
are resolved **shell-side from the token** — un-spoofable, never cell-set. This layer is driven by:
- `ui/src/lib/vars/*.test.ts` — the pure interpolation unit suite (all three syntaxes, format hints
  `${var:json|csv|singlequote|pipe}`, multi-value expansion, every built-in, unknown-var-left-literal).
- `DashboardView.gateway.test.tsx` — the URL round-trip (`?var-site=…&from=…&to=…&refresh=…` parses
  → selection → cells re-run against the **real** gateway), and the identity-un-spoofable assertion
  (an iframe/postMessage value for `${__user.*}` is ignored; the shell's token value wins).

You **observe** this layer live by opening the running app (Step 4): pick a site in the variable
bar, watch the cell re-query, copy the URL, and confirm it reproduces the selection.

### 2.4 Permissions — the negative path

```bash
curl -s $BASE/dashboards -o /dev/null -w "%{http_code}\n"   # NO token → 401
```

- **Per-verb cap deny:** a token missing `mcp:dashboard.save:call` (or `panel.save`, or
  `dashboard.access_check`) is refused server-side. Dev-login carries the save caps, so the deny case is
  the `signInWithCaps` gateway/cargo test — a token narrowed below the verb.
- **Editing the surface is admin-only** (viewer mode): the shell gate is `canEdit = isAdmin(caps)`, **not**
  `hasCap(dashboard.save)` (every member holds `dashboard.save`, so gating the UI on it made everyone an
  editor — the bug viewer-mode fixed). A **viewer** reads the live grid but gets no authoring surface
  (no roster, no drag/resize, no ⚙, no add-panel); an **admin** gets all of it. The server-side proof
  that "a viewer can't edit" is the ordinary per-verb deny (a token below `dashboard.save`/`.delete`) —
  and since `/login` always grants the full dev claim set, that lives in
  [`DashboardView.gateway.test.tsx`](../../../ui/src/features/dashboard/DashboardView.gateway.test.tsx)
  (VIEWER / ADMIN / VIEWER-DENY trio), not a live curl.
- **Variable/JSON-sink leash:** a variable's query or a JSON-payload target can only call a tool in the
  cell's tool set ∩ install grant, re-checked at the host. A payload targeting an ungranted tool (or a
  `bus.publish` to a reserved/cross-ws subject) is refused — the headline deny of the vars scope.

### 2.5 Access — the workspace wall (mandatory)

```bash
# seed a dashboard + a var query in acme
curl -s -X POST $BASE/dashboards -H "$A" -H "$C" \
  -d '{"id":"iso-dash","title":"secret","cells":[],"variables":[]}' -o /dev/null
# a globex token must NOT see it — and its variable query reaches only globex data
TB=$(curl -s -X POST $BASE/login -H "$C" -d '{"user":"user:bob","workspace":"globex"}' | jq -r .token)
curl -s $BASE/dashboards/iso-dash -H "authorization: Bearer $TB" -o /dev/null -w "%{http_code}\n"  # 404
curl -s $BASE/dashboards         -H "authorization: Bearer $TB"                                    # [] (no acme rows)
curl -s -X DELETE $BASE/dashboards/iso-dash -H "$A" -o /dev/null   # remove the isolation SCAFFOLD only
```

**Observe:** `globex` gets `404` + an empty list; the wall is checked **before** caps. A ws-B viewer of
a shared dashboard sees **their own** `${__user}`/`${__workspace}` (token-resolved), and a variable query
run in ws-B reaches only ws-B's `demo-buildings` grant (registered per-workspace). A URL `?var-` value
can't cross the wall — it's just a selection over ws-B's own options.

---

## Step 3 — on a wrong result, diagnose the seam in order

1. **Variable options empty?** Is `demo-buildings` seeded + registered (`curl /datasources`)? An
   unregistered source ⇒ the `building` query-var resolves to nothing — not a vars bug. Re-run Step 0.
2. **`${site}` renders literally in a cell?** Unknown/unbound variables interpolate to **themselves**
   (Grafana behavior, a soft warning) — either no `?var-site=` selection and no default, or the var
   name is misspelled. For a `required` var this is the intended "select a Building" gate, not a bug.
3. **`access_check` says `unchecked`, not `ok`?** v1 marks a held-cap query-var `unchecked` (per-value
   option resolution isn't walked) — that's honest, not a failure; `unchecked` keeps the overall report
   from claiming green. A `bad`/`ok:false` is the real gap.
4. **Stale node?** `make kill && make dev` after a Rust change (memory: flows-dev-node-no-hot-reload).
5. **`now` missing on `/mcp/call`?** The REST routes fill the clock; `/mcp/call` requires `now` in args
   (determinism) — omit it and the verb errors. The `access_check`/`save` examples pass it explicitly.

Only once the seam is real and still wrong: open `../../debugging/frontend/<symptom>.md` (or
`.../dashboard/…`) per [`../../scope/debugging/debugging-scope.md`](../../scope/debugging/debugging-scope.md),
find the root cause, add a **regression test** (`DashboardView.gateway.test.tsx` for a UI/URL seam, or
`cargo test -p lb-host …` for a store/access_check seam — fails-before/passes-after), update
`debugging/README.md`.

---

## Step 4 — what to leave behind (definition of done)

- Proof the source was **seeded** (`/datasources` shows `demo-buildings`), the dashboard + panel + vars
  **round-tripped** (the read-backs), and the **access-check verdict** for the required variable — in the
  session doc. Green is a claim you show.
- CRUD + panels/ref-cell + variables (all three layers: definition round-trip, the required-var access
  gate, and the URL/interpolation layer via the named tests) + permissions (per-verb deny, viewer-mode,
  the variable/JSON leash) + access (the workspace wall) covered.
- **Left inspectable:** `keep-dash` (the ref-cell dashboard drawing seeded energy data) and
  `e2e-site-vars` (the **site** variable-bar dashboard) still in place, the `energy-by-building` panel
  still registered, the `demo-buildings` source still registered, the node still running. Your **final
  response hands the user the exact page** — e.g. "open **`e2e-site-vars`** at
  http://127.0.0.1:8080/#/t/acme/dashboards — pick a site in the variable bar and watch the chart
  re-query the seeded data; the URL updates to `?var-site=…` so you can share the exact view. I left it +
  `keep-dash` in place so you can check."
  Do **not** delete them; only the throwaway/scaffold rows (`e2e-dash`, `iso-dash`) are removed.
- On any failure: a completed `debugging/frontend/…` (or `.../dashboard/…`) entry + regression test,
  cross-linked.

---

## Related

- The seeded datasource the panel + variable query read: [`../datasources/README.md`](../datasources/README.md).
- Charts over the node's own series (no external source): [`../charts/README.md`](../charts/README.md).
- A rule as a panel source (a rule's `output` drives a widget): [`../rules/README.md`](../rules/README.md).
- The dashboard/panel drivable surface: [`../../skills/dashboard-mcp/SKILL.md`](../../skills/dashboard-mcp/SKILL.md),
  [`../../skills/panels/SKILL.md`](../../skills/panels/SKILL.md).
