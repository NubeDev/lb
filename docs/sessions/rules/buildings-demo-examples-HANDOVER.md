# HANDOVER — Buildings demo rule examples (INCOMPLETE, needs real testing)

Status: **NOT DONE.** Examples were edited and hand-verified with *throwaway* tests that were
deleted. There is **no committed regression test**. Do not trust the examples as "shipped" until
the testing below exists and is green.

---

## The user's ORIGINAL GOAL (do not lose this)

Design a **beginner lesson / guide** for the Lazybones platform, taught against the seeded demo
datasource **`.lazybones/data/demo/buildings.db`** (a SQLite building-energy dataset:
`site → meter → point → point_reading`, Haystack-tagged; 8 buildings, 30 days @ 15-min). The lesson
walks a true beginner (an END USER, not an AI, not a coder) through the platform's feature chain:

1. **datasource + query** (user said they don't need help here — start from a query)
2. **write rules**
3. **flows**
4. **insights**
5. **dashboard** (adding a new panel)

"Keep it pretty basic, it's for beginners." Each step must use **real data from buildings.db** and
show the **real result** — no invented series names (an earlier draft used `cooler.temp`, which is
NOT in the DB; the user rejected that hard).

The lesson does not exist yet. What follows (rule examples) is only a fragment of step 2.

---

## What was actually touched

**One file:** `ui/src/features/rules/examples/examples.ts` — added 3 click-to-load rule examples
(ids `buildings-intensity-query`, `buildings-intensity-strict`, `buildings-intensity-alert`) that
query `demo-buildings` and (optionally) raise a finding. All 3 run the SAME query; they differ only
in the finding block (commented out in the first two, live `alert` in the third).

Nothing else is committed. `git status` shows only this file modified (plus unrelated pre-existing
WIP in `rust/crates/rules/` — a `chart` verb — that is NOT mine).

---

## Hard-won facts (verified against the REAL engine, keep these — they cost hours)

The demo query runs through **`federation.query` → DataFusion 53**, NOT SQLite. SQLite is only the
storage file; the query planner is DataFusion and is much stricter. Every one of these was a real
failure caught only by running the full path:

1. **Rhai strings can't span lines.** A multi-line `"..."` → `Open string is not terminated`.
   FIX: put multi-line SQL in a **backtick raw string** `` `...` `` (Rhai 1.25 supports it).

2. **DataFusion requires every non-aggregate SELECT column in GROUP BY.** SQLite allows
   `SELECT s.name ... GROUP BY s.id`; DataFusion rejects it (`Column ... must appear in the GROUP BY`).
   FIX: `GROUP BY s.id, s.name, a.val`.

3. **DataFusion won't take a correlated subquery in SELECT under a wildcard/group.**
   FIX: get `area` via a real `JOIN site_tag a ON a.site_id=s.id AND a.tag='area'`, not a subquery.

4. **Use `CAST(... AS DOUBLE)`**, not `AS REAL` (DataFusion's type name).

5. **`.records()` returns rows as POSITIONAL ARRAYS, not maps.** A row is `["Riverside...", 4.68]`.
   `r.building` / `r.kwh_per_m2` → `Unknown property ... for type 'array'`. FIX: `r[0]`, `r[1]`
   (SELECT order). `.columns()` gives the names.

6. **`rules.run` output is scalar-wrapped:** the last-expression array is at
   `output.value`, not `output` directly (only matters for a test asserting row count).

7. **`alert(#{...})` requires caps `mcp:inbox.record:call` + `mcp:outbox.enqueue:call`** — the host
   routes an alert finding to the inbox + outbox after the run. `emit(#{...})` does NOT (stays in
   the result). A rule caller without those caps → `denied` on any body that calls `alert`.

8. **There is NO `insight.*` verb in the rule engine.** A rule can only `emit`/`alert` (→ findings /
   inbox+outbox), `log`, `inbox.*`, `outbox.*`, `channel.*`. A true `insight:` record (open→ack→
   resolved) comes ONLY from the **`insight.raise` MCP verb** or a **flow's insight-sink node** —
   NOT from a rule body. If the lesson's "insights" step needs a rule to raise an insight, that is
   the FLOW path, not a rule. (User asked for a rule→insight example; the honest answer is
   emit/alert, or move insight-raising to the flow step.)

9. **The `// ---` comment trap (the user's repeated pain).** Comment headers written as `// --- ... ---`
   become a bare `--- ...` when the `//` is stripped during uncommenting, and Rhai reads `--` as a
   reserved operator → `'--' is a reserved symbol`. **NEVER put `--` (or `---`) inside example
   comments.** This bit the user 2-3 times. The last edit still needs this scrubbed from the shipped
   file (see "Immediate TODO" #1).

### The proven-good query (ran e2e, 8 rows, Riverside Data Center = 4.68 kWh/m² on top)

```
SELECT s.name AS building,
  ROUND(SUM(pr.value) / CAST(REPLACE(a.val,' m2','') AS DOUBLE), 2) AS kwh_per_m2
FROM point_reading pr
JOIN point p ON p.id = pr.point_id
JOIN meter m ON m.id = p.meter_id
JOIN site  s ON s.id = m.site_id
JOIN site_tag a ON a.site_id = s.id AND a.tag = 'area'
WHERE p.name = 'Energy kWh'
GROUP BY s.id, s.name, a.val
ORDER BY kwh_per_m2 DESC
```

### Real numbers in the DB (for lesson copy)

- Energy intensity: **Riverside Data Center 4.68 kWh/m²** — 6× the next building (Westend/Northside 0.79).
- Comfort: **Southbank Office** zone temp over 24°C on all 2880 readings; Eastfield next (2477).
- Point types present: Energy kWh, Demand kW, Zone/Supply/Return (Air) Temp, Valve Position,
  Fan/AC/Pump/Plant/Compressor Status, Mode, Flow L/min, Total m3, Gas kWh, HVAC Energy/Demand.

---

## Immediate TODO (finish step 2 properly)

1. **Scrub the `---` from `examples.ts`.** The three buildings examples still have `// --- ... ---`
   header lines. Rewrite those comment lines WITHOUT any `--`/`---` (plain words). This is the exact
   thing that keeps breaking for the user on uncomment.

2. **Write a COMMITTED regression test** (rule 9: no mocks, real path). It must:
   - install the REAL `federation` sidecar, `datasource.add` the REAL `buildings.db`
     (kind `sqlite`, dsn = absolute path), then run each example body through the REAL `rules.run`.
   - Model it on `rust/crates/host/tests/federation_sqlite_test.rs` (install + add_source harness)
     merged with a `rules.run` call. The caps the principal needs:
     `mcp:federation.query:call`, `mcp:rules.run:call`, and for the alert body
     `mcp:inbox.record:call` + `mcp:outbox.enqueue:call`.
   - Assert: query/strict bodies → 8 rows out, 0 findings; alert body → 8 rows, 1 finding
     (Riverside). Include the MANDATORY capability-deny + workspace-isolation cases (testing scope).
   - The example bodies should be the SOURCE OF TRUTH the test reads — ideally the test imports the
     same strings, or a contract-mirror guard keeps them in sync, so an edit to `examples.ts` that
     breaks the query fails CI. (The whole disaster this session was examples drifting from a tested
     path. Fix that structurally.)

3. **Only after 1+2 are green**, treat step 2 (rules) as done and move to the rest of the lesson.

---

## The rest of the lesson (NOT started — the actual goal)

Design + write the beginner guide covering all 6 steps against buildings.db. Verified mechanics so far:

- **datasource:** `datasource.add {name, kind:"sqlite", endpoint, dsn}` / `.list` / `.test`;
  read-only; DSN is a node-local file path. Seed+register via `make seed-demo-sqlite WS=<ws>` →
  registers id `demo-buildings`.
- **query:** authored in PRQL or raw SQL; `query.run`/`query.compile`; `target` = `platform`
  (store.query) or `datasource:<name>` (federation.query). UI: `query-workbench` + `data-studio`.
  Chart-ready shape = first column `time` + numeric columns, `ORDER BY time`.
- **rules:** Rhai body via `rules.run`/`rules.save`; `query()/source()/history()` in, `emit/alert/log`
  out; findings render in the FindingsList. UI: `ui/src/features/rules` (RuleEditor, examples/).
- **flows:** a `rhai`/`rule` node runs a rule body inside a flow; the node emits `{output, findings,
  log}` onto the message envelope for the next node. `insight` sink node raises a real insight.
  UI: `ui/src/features/flows`. NOT yet verified e2e here.
- **insights:** `insight.raise/list/ack/resolve`, severity + open→ack→resolved lifecycle,
  dedup-keyed occurrences. Package `@nube/insights` (`packages/insights`), widgets bind on a
  dashboard. NOT yet verified e2e here.
- **dashboard/new-panel:** panel-builder wizard (SourceStep → ChartTypeStep → OptionsStep); a panel
  binds to a source of type `federation` (a datasource query), `series`, `surreal`, `insights`, or
  `flows`. UI: `ui/src/features/panel-builder`, `ui/src/features/dashboard/builder`. NOT yet verified.

Deliverable format still undecided with the user (repo MDX under `doc-site/content/public/` vs. a
visual page). The user wants it hands-on, plain-words, beginner-level, real data throughout.

## Test commands
- Rust e2e: `cd rust && cargo test -p lb-host --test <name>` (federation sidecar builds on demand).
- UI: `cd ui && npx vitest run src/features/rules` ; `npx tsc --noEmit -p tsconfig.json`.
- Inspect data: `sqlite3 .lazybones/data/demo/buildings.db`.
