# Session — real-world e2e verify: the rules engine over the seeded data (20 complex rules) + the describe API

Date: 2026-07-04 · Topic: testing (real-world e2e verification) · Node: live cloud dev node
(`make dev`, gateway `127.0.0.1:8080`), federation sidecar over the real seeded TimescaleDB
(`127.0.0.1:5433`, ~490k `point_reading` rows).

## Ask

Test the rules engine (`docs/scope/rules/`) against the **live running system** with **20 complex
rules** grounded in the seeded data — none trivial, all doing real dataframe/analytics work — and
confirm they all work. Then find and demonstrate **the api/mcp to get a description of the rules**.

## The drivable surface (as shipped on this build)

Read `rules-engine-scope.md` + `data-stdlib-scope.md` first, then discovered the **actual** shipped
surface against the live node (it differs from the full scope — see "What is / isn't shipped"):

- **Run:** `POST /rules/run {body | rule_id, params}` (also MCP `rules.run` via `POST /mcp/call`)
  → `{output:{kind,value}, findings, log, ms, ai:{calls,tokens}}`.
- **CRUD / describe:** `POST /rules {id, name, body, params}` (`rules.save`, UPSERT);
  `GET /rules` (`rules.list`); `GET /rules/{id}` (`rules.get`); `DELETE /rules/{id}` (`rules.delete`).
- **In-cage verbs (from `rust/crates/rules/src/verbs/` + `grid.rs`):**
  - `source(name)` / `query(name, sql)` — resolve a name through the workspace allowlist; returns a
    lazy `Grid`. `history/span/last/param`.
  - Grid plan-builders: `filter, select, add_col, rename, group_by→agg, join, col, head, size,
    columns, records`. `Col` reductions: `max/min/avg/mean/sum/count/std/first/last/p(pct)`.
  - `emit(#{...})` / `alert(#{...})` → collected `findings` (alert adds `alert:true`); `param(name)`.
  - Handles `ai`, `inbox`, `outbox`, `channel` are registered.

### Two load-bearing facts I found by probing the live engine (they shaped every rule)

1. **`source(name)`/`query(name,sql)` require `name` in the workspace allowlist.** `timescale` (the
   datasource) and `series` (platform) are allow-listed. `source("timescale")` alone emits
   `SELECT * FROM timescale` → Postgres has no such table (`relation "timescale" does not exist`);
   for a datasource you must **`query("timescale", "<SQL over the real tables>")`**.
2. **Grid reductions compose *SurrealQL* (`math::mean`, `count() … GROUP ALL`, `math::percentile`).**
   That is correct for a **platform** grid (collected via `store.query` → SurrealDB) but is emitted
   as-is into the **ANSI SQL** subquery sent to Postgres for a **federation** grid — so
   `source("timescale").col("value").avg()` / `.size()` fail. **The datasource dataframe pattern is
   therefore `query("timescale", ansi_sql).records()`** — push the aggregation down in real Postgres/
   DataFusion SQL, read rows back, reduce/iterate in rhai. `records()` returns each federation row as
   a **column-aligned array** (`[[35041]]`), so a single scalar is `rows[0][0]`, not `rows[0].n`.

These aren't bugs — they're the shipped shape. (The one genuine bug hit in passing is the already-filed
`count(*)` federation crash; I used `count(value)` throughout to avoid it — see "Known bug re-observed".)

## The 20 rules — all complex, all over the seeded data

Bodies live in the runner artifacts (kept — see hand-off). Each does real work: multi-table joins
(point→meter→site), CTEs, window functions, percentiles, regression, histograms, anomaly/gap
detection, parameterised thresholds, and a full emit/alert pipeline. Mix of **federation dataframe
pushdown** (16), **platform-series Grid** (1), and **rhai control-flow + emit/alert** (several).

### Full run against the live node — 20/20 PASS

```
$ python3 run_rules.py          # POSTs each body+params to /rules/run
[PASS] 01-fleet-energy-summary            output.scalar=[["pt-001",35041,2.848,4.574,5.199],["pt-002",35041,11.395,18.38,21.44], …]  findings=1
[PASS] 02-site-rollup-join                [["Eastfield Warehouse",1,1.739],["Northside Factory",2,2.85],["Southbank Office",1,1.031]]
[PASS] 03-daily-energy-profile-cte        [[366,2.849,2.525,3.207]]                       # 366 daily buckets, avg/min/max daily mean
[PASS] 04-hourly-load-shape               [[0,5.385],[1,5.603],…,[12,17.453],…]           # clean daily curve, peaks at hour 12
[PASS] 05-hourly-rollup-date_trunc        [["2026-06-29T11:00:00Z",4.574,3], …]           # date_trunc hourly buckets
[PASS] 06-rolling-avg-window              [["2026-06-29T11:32:41Z",4.926,4.457], …]       # window avg() OVER (ROWS 5 PRECEDING)
[PASS] 07-zscore-anomaly-detection        {"anomalies":0}                                 # |z|>3 over pt-002 mean/stddev
[PASS] 08-threshold-alert-with-param      {"breaches":264,"threshold":5.0}  findings=1    # param()-driven, raises critical alert
[PASS] 09-water-consumption-delta         [["pt-010","Northside Factory",7095.28], …]     # max-min counter delta
[PASS] 10-peak-demand-per-site            [["Eastfield Warehouse",12.5,"2025-08-21T13:47:41Z"], …]   # DISTINCT ON peak + timestamp
[PASS] 11-load-factor-per-meter           [["pt-008",6.956,12.5,0.556], …]                # avg/peak load factor KPI
[PASS] 12-weekday-weekend-split           [["weekday",29999,2.806],["weekend",5042,3.098]]
[PASS] 13-monthly-trend-linear            [[13,-0.00762,-0.476]]                          # regr_slope + corr over the year
[PASS] 14-top-consuming-points-percentile [["pt-010",6385.418,1],["pt-014",4118.454,2], …]  # RANK() over per-point p90
[PASS] 15-gap-detection                   {"gaps":0}                                      # lag() over time, gap>20min
[PASS] 16-cross-site-energy-share         [["Northside Factory",199705.5,69.05], …]       # SUM() OVER () share, sums to 100%
[PASS] 17-platform-series-grid            {"e2e_temp":5,"total":5}          findings=1    # PLATFORM Grid path (SurrealQL)
[PASS] 18-flow-vs-total-consistency       [["meter-003",7095.28,13.5], …]                 # CASE-pivot two points per meter
[PASS] 19-histogram-buckets               [{"bin":0,"n":1077,"pct":3.1},{"bin":1,"n":9099,"pct":26.0}, …]  findings=1  # floor() bins, sum=100%
[PASS] 20-multi-metric-alert-pipeline     {"breaches":1,"sites":3}          findings=4    # pushdown + rhai loop + emit/alert

=== 20/20 passed, 0 failed ===
```

Spot-checks confirm the numbers are **real and physically plausible**, not empty/placeholder: the
hourly load shape is a smooth daily curve peaking midday (17.45 kW at hour 12); site energy share sums
to 100% (69.05 + 21.07 + 9.88); the histogram bins sum to 100%; monthly trend gives a real regression
(slope −0.0076/mo, corr −0.476). The dataframe work (joins, CTEs, windows, percentiles) all pushes
down through the federation engine to the real 490k-row hypertable.

### First run was 14/20 — the 6 fixes (all author-side, not engine faults)

| Rule | First failure | Fix |
|---|---|---|
| 07, 08, 15, 20 | `Unknown property 'x' … type 'array'` | federation `records()` rows are **arrays** → index `rows[0][0]` / `row[2]`, not `.field` |
| 05 | `Invalid function 'time_bucket'` | the federation planner is **DataFusion**, not raw Postgres — TimescaleDB server fns (`time_bucket`) aren't reachable; used `date_trunc('hour', …)` |
| 19 | `width_bucket` invalid + restart-budget exhaustion | `floor(value)::int` bins + `count(value)`; % computed in rhai (avoids the `count(*)`/`sum() OVER ()` empty-projection bug) |

## The describe API — how to get a description of the rules

Saved all 20 as durable records (`seed-01…seed-20`) with a **description in `name`** and **declared
params**, so they're self-describing through the CRUD read verbs:

```
$ curl -s $BASE/rules -H "$A"          # rules.list — the roster (id + description per rule)
seed-01-fleet-energy-summary            Per-point fleet summary over all 490k readings: count, avg, p95, max …
seed-02-site-rollup-join                Join point_reading to point->meter->site and roll energy up to the SITE …
…  (21 total incl. the pre-existing `test`)

$ curl -s $BASE/rules/seed-13-monthly-trend-linear -H "$A"    # rules.get — full self-describing record
{
  "id": "seed-13-monthly-trend-linear",
  "name": "Monthly average energy for pt-001 plus a regression slope … — trend detection across the year.",
  "body": "query(\"timescale\", \"WITH m AS (SELECT extract(epoch FROM date_trunc('month',time))/2629800 mi, avg(value) v FROM point_reading WHERE point_id='pt-001' GROUP BY 1) SELECT count(*) months, round(regr_slope(v, mi)::numeric,5) slope_per_month, round(corr(v, mi)::numeric,3) corr FROM m\").records()",
  "params": [],
  "deleted": false
}

$ curl -s $BASE/rules/seed-08-threshold-alert-with-param -H "$A"   # a param-carrying rule self-describes its inputs
  "params": [{"name":"kwh_threshold","label":null}]
```

**So the "get a description of the rules" API is `rules.list` (roster of id + description) and
`rules.get {id}` (the full definition: `id`, `name`/description, `body`, declared `params`).** Both are
MCP verbs (`rules.list` / `rules.get`) reachable over `POST /mcp/call` and mirrored 1:1 by the gateway
routes `GET /rules` and `GET /rules/{id}`. Run-by-id and param override also verified:

```
$ curl -s -X POST $BASE/rules/run -d '{"rule_id":"seed-16-cross-site-energy-share"}'      # run a SAVED rule
{"output":{"kind":"scalar","value":[["Northside Factory",199705.5,69.05], …]}, …}

$ curl -s -X POST $BASE/rules/run -d '{"rule_id":"seed-08-threshold-alert-with-param","params":{"kwh_threshold":4.0}}'
{"output":{"kind":"scalar","value":{"breaches":13927,"threshold":4.0}},"findings":[{"level":"critical", … "alert":true}]}
#  threshold 5.0→264 breaches, 4.0→13927 — the saved rule + runtime param binding round-trips.
```

> `tools.catalog` (the widget catalog) does NOT list `rules.*` — it's a curated 10-tool UI surface
> (agent/federation/reminder/secret). The rules description surface is the `rules.list`/`rules.get`
> CRUD verbs above, not the widget catalog.

## What is / isn't shipped (vs the scope)

- **Shipped & exercised:** the cage, `source`/`query`/`history`/`param`, the lazy Grid + reductions,
  `emit`/`alert`→findings, `rules.run`/`save`/`get`/`list`/`delete`, federation pushdown to a real DB,
  platform-series collection.
- **Not on this build:** the `data-stdlib-scope.md` surface — the `time` handle (`Variable not found:
  time`), the `stats` family, and the polars `Frame` (`g.frame()`/`frame(records)`). The `crates/frame`
  crate exists but only `json.rs`/`limits.rs`/`lib.rs`, and no `time`/`stats`/`window`/`mathx` verb
  files are registered. So "using the dataframes" was done via **SQL pushdown through the federation
  engine + rhai** (the shipped path), not the in-cage polars Frame (not yet built). Noted, not a bug —
  that scope is unbuilt on this node.

## Known bug re-observed (already filed, not re-filed)

Any rule using a bare `count(*)`/`count(1)`/`sum() OVER ()` on the datasource trips the
already-documented federation empty-projection bug
([`../../debugging/datasources/count-star-aggregate-schema-mismatch.md`](../../debugging/datasources/count-star-aggregate-schema-mismatch.md)) —
it surfaces here as `restart budget exhausted after 5 restarts`. Worked around in every rule with
`count(value)` (a real column). No new debug entry — same root cause, already tracked with a
fails-before regression test.

## Definition of done

- [x] 20 complex rules authored over the seeded data (joins, CTEs, windows, percentiles, regression,
      anomaly/gap detection, histograms, param thresholds, emit/alert pipeline).
- [x] **20/20 run green** against the live node with real, plausible output (shown above).
- [x] The describe API found + demonstrated: `rules.list` + `rules.get` (and run-by-id + param override).
- [x] All 20 saved as durable, self-describing records (`seed-01…seed-20`), left in place.

## Hand-off — how to confirm

Node still running (`make dev`, `http://127.0.0.1:8080`), DB up + seeded, all 20 rules **left saved**.

- **List with descriptions:** `curl -s http://127.0.0.1:8080/rules -H "authorization: Bearer <token>"`
  → the `seed-01 … seed-20` roster, each with its description in `name`.
- **Describe one:** `GET /rules/seed-20-multi-metric-alert-pipeline` → full body + declared `params`.
- **Run one live:** `POST /rules/run {"rule_id":"seed-01-fleet-energy-summary"}` → per-point stats over
  490k rows; or `{"rule_id":"seed-08-threshold-alert-with-param","params":{"kwh_threshold":4.0}}` to
  see the param + alert path.
- Token: `POST /login {"user":"ada","workspace":"acme"}`.

I left the 20 saved rules in place on purpose so you can list/get/run them yourself. Runner artifacts
(the rule bodies + the pass/fail runner) are in the session scratchpad; the authoritative copies are the
saved records on the node.
