---
name: rules
description: >-
  Author and run Lazybones rules over the node gateway — a sandboxed Rhai script that reads data
  (platform series/store or an external datasource), transforms it (a lazy column `Grid`: filter/
  rollup/join/reduce), optionally calls AI, and emits findings/alerts. Save/get/list/delete/run rules
  via `rules.*`. Use when a task says "write/run a rule", "run a Rhai rule in the Playground", "author
  a threshold/alert rule over series", "save a rule", or "call rules verbs over REST/MCP". The engine
  is an in-process `lb-rules` library (ported from rubix-cube), capability-first: the Rhai cage reaches
  nothing but the host's gated verbs; a run cannot widen beyond what its invoker holds.
---

# Authoring & running rules (`rules.*`, the `lb-rules` engine)

A rule is a small **sandboxed Rhai script** that reads data through the platform's gated verbs,
composes a lazy column-oriented `Grid` (filter/rollup/align/join/reduce), optionally calls `ai.*`
through the AI-gateway (with a per-run budget + an nsql re-validation fence), and **emits findings /
raises alerts** (into the inbox, must-deliver notifications through the outbox). The engine is an
**in-process library** (`rust/crates/host/src/rules/`, ported from `rubix-cube`) — not an external
service — and that is exactly why it fits: the Rhai cage can reach neither the store, the bus, nor a
socket; only the verbs, each running the host's `caps::check`.

Two call styles, as with the other host surfaces:

1. **Dedicated REST routes** — the Playground's surface (`/rules…`).
2. **The universal MCP bridge** — `POST /mcp/call {tool, args}` by dotted name.

Both derive **workspace + principal from the token** (host-set, never script-set). Every verb is
capability-gated; denials are opaque.

## 1. Authenticate

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

Capabilities — one per verb: `mcp:rules.run:call`, `rules.save`, `rules.get`, `rules.list`,
`rules.delete`, `rules.help`. **A run cannot widen its invoker (`caller ∩ grant`):** inside the run, every data verb
(`data.query`/`series.*`/`federation.query`) hits the host `caps::check` under the *invoker's*
authority — a rule reading a source the caller lacks is denied mid-run, even though the body is valid.
`ai.*` needs the AI-gateway cap.

## 2. The verbs

| Action | REST route | MCP bridge (`POST /mcp/call`) | Args |
|---|---|---|---|
| Run (ad-hoc or saved) | `POST /rules/run` | `{"tool":"rules.run","args":{…}}` | `body` \| `rule_id`, `params?`, `ts?` |
| Save (create/update) | `POST /rules` | `{"tool":"rules.save","args":{…}}` | `id`\|`name`, `name?`, `body`, `params?` |
| Get one | `GET /rules/{id}` | `{"tool":"rules.get","args":{"id":"…"}}` | `id` |
| List | `GET /rules` | `{"tool":"rules.list","args":{}}` | — |
| Delete | `DELETE /rules/{id}` | `{"tool":"rules.delete","args":{"id":"…"}}` | `id` |
| **Catalog** | — | `{"tool":"rules.help","args":{}}` | — |
| **Run in background** | — | `{"tool":"rules.run_async","args":{…}}` | `body` \| `rule_id`, `params?`, `ts?`, `run_id?` |
| Run: status | — | `{"tool":"rules.runs.get","args":{"run_id":"…"}}` | `run_id` |
| Run: list | — | `{"tool":"rules.runs.list","args":{…}}` | `status?`, `limit?` |
| Run: pause | — | `{"tool":"rules.runs.suspend","args":{"run_id":"…"}}` | `run_id` |
| Run: resume | — | `{"tool":"rules.runs.resume","args":{"run_id":"…"}}` | `run_id` |
| Run: cancel | — | `{"tool":"rules.runs.cancel","args":{"run_id":"…"}}` | `run_id` |

- **`rules.run`** takes EITHER an inline `body` (ad-hoc Playground run) OR a saved `rule_id`, plus
  `params` (bound into the script) and `ts` (a logical clock — determinism, no wall-clock). It's the
  hot path: **bounded by the governors, so it's a synchronous call** returning
  `{output, findings, log, ms, ai}`. Long/batch/resumable work is **`rules.run_async`** (§8), not a
  `rules.run` loop.
- **`rules.save`** upserts `rule:{ws}:{id}` — `id` is the stable key (defaults to `name`); `body` is
  the Rhai source; `params` is a typed declared-param list. A save does NOT execute the body.
- **`output.kind`** is one of `scalar | grid | findings | nothing`.
- **`rules.help`** is the introspection verb: it returns the **function catalog** (the single source
  of truth, `lb_rules::CATALOG`) — `{"functions":[{name,family,signature,description}, …]}` — so an
  agent or the UI can discover every available in-cage function and its description without reading
  this doc. Gated `mcp:rules.help:call` like the others.

## 3. Writing a rule

To the author, `source("cooler.temp")` (platform) and `source("demo-buildings")` (external) read
alike — one `source(...)` surface, two correct paths beneath (SurrealDB-native `series.*`/`data.query`
vs the `federation` extension's `federation.query`). The `Grid` stays lazy (composed query + context;
materializes only on `collect`/a `Col` reduction/`return`), so chaining never copies data.

> **External-source examples need a registered datasource first.** The `demo-buildings` source used
> below is the Docker-free SQLite building dataset — seed + register it in one command against a
> running node: `make dev` then `make seed-demo-sqlite` (generates `.lazybones/data/demo/buildings.db`
> and `datasource.add {kind:"sqlite", …}` in `acme`). See
> [`../../testing/datasources/README.md`](../../testing/datasources/README.md) Step 0. Until then a
> rule that reads `demo-buildings` is denied at the `source(...)` seam — the source doesn't exist yet.

```bash
# ad-hoc run of a threshold+alert rule (the Playground path)
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"rules.run","args":{"ts":1719800000000,"body":
"let hot = source(\"cooler.temp\").last(\"24h\").rollup(\"1h\", \"max\").filter(\"max > 5.0\");
 if hot.size() > 0 { alert(#{ level: \"critical\", series: \"cooler.temp\", msg: \"over 5C\" }); }"}}'
# → {"output":{"kind":"findings"},"findings":[{…}],"log":[…],"ms":…,"ai":{…}}

# save it, then run by id
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"tool":"rules.save","args":{"id":"cooler-foodsafety","name":"Cooler food-safety","body":"…"}}'
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"rules.run","args":{"rule_id":"cooler-foodsafety"}}'
```

Verb families available in the cage (ported from rubix-cube): **data** (`source`/`query`/`dataset`/
`history`/`span`/`last`/`param`), **timeseries** (`rollup`/`align`/`interpolate`/`gapfill`/`resample`/
`lag`/`delta`/`rate`), **Grid** reductions (`filter`/`select`/`add_col`/`rename`/`group_by`/`join`/
`col`/`size`/`head`), **ai** (`ai.ask`/`complete`/`classify`/`embed` — metered + fenced), and
**emit** (`emit`/`alert`/`log`). **The authoritative list — name, family, signature, description for
every function — is `rules.help`** (the catalog in `lb_rules::CATALOG`); this paragraph is a map, the
catalog is the territory.

### `ai.*` runs against the workspace's SELECTED model (rules-ai-wiring)

`ai.*` in a rule reaches the **same model the workspace picked for its agent** — the agent-catalog
selection written to `agent.config` (Settings → Agent). The `rules.run` bridge resolves that pick to
the node's model and drives a **single-turn** completion (no tools, no agent loop): `ai.complete` is
one model turn; `ai.ask` asks the model to propose SQL, which is then **re-fenced** through the same
`collect` a hand-written `query()` takes before it can run. A rule never gets the agent's tool-calling
loop — its data/emit power comes from the *rule verbs*, gated by the cage + caps.

```bash
# a configured workspace: ai.complete reaches the real model, the result lands in `findings`
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"rules.run","args":{"ts":1,"body":
"let summary = ai.complete(\"which coolers ran hot today?\"); emit(#{ summary: summary });"}}'
# → {"output":{"kind":"findings"},"findings":[{"summary":"…the model answer…"}], "ai":{"calls":1,…}}
```

**Honest "AI configured?" — the model is real only when BOTH hold:** (1) the workspace **selected** a
model endpoint in `agent.config` (the catalog pick), and (2) the node has a **real provider** wired
(not the boot placeholder). Either missing → `ai.*` returns the clear `"AI not configured for rules"`
error, while a **data-only rule still runs**. Today only the sanctioned deterministic `MockProvider`
exists; a rule's `ai.ask` does not hit a live LLM until a real provider adapter is configured — do not
promise otherwise in a workbench prompt. Picking a *different* definition in the catalog changes which
model a subsequent rule run uses — the pick is the single source of truth for "which model do my rules
use", exactly as for the agent.

## 4. The cage (why a rule is safe)

Security is "absence of capability + presence of limits", in-process:

- **No I/O surface** — the engine registers no file/net/process API; `eval`/`import` are disabled,
  `set_max_modules(0)`. A rule can do nothing but call the verbs handed to it.
- **Resource governors** — `max_operations`, call-level depth, string/array/map size caps, and a
  wall-clock deadline bound *work and time* (DoS defense), while `caps::check` bounds *authority*.
- **`ai.*` invariants** — a per-run **budget meter** (call + token caps; a `for`-loop of `ai.complete`
  can't overspend) and the **nsql fence** (a model-proposed query is re-validated through the same gate
  a hand-written one is, before it runs — no nsql path skips the validator).
- **`alert` is real** — it raises an **inbox** item and routes a must-deliver notification through the
  **outbox**, never raw pub/sub or a sandbox-internal marker. `emit` collects a finding; `log` records
  a line.

## 5. Propose & approve — gate an effect on a human sign-off (rules-approvals)

A rule can **propose a change and stage the effect it will fire only if a human approves** — "a rule
proposes, a human disposes". `inbox.request_approval` raises a `needs:approval` item AND durably stages
the `on_approve` effect in a **held** state; the outbox relay skips a held effect, so it is *never*
delivered until the item is approved:

```rhai
// Raise an approval item AND stage the page it sends IF approved (staged HELD, not delivered yet).
inbox.request_approval(#{
    id: "refund-proposed",
    channel: "ops",
    body: "Refund proposed — cooler breached",
    route: "team:managers",                              // who should sign off (advisory in v1)
    on_approve: #{ target: "notify", action: "page",     // the HELD effect
                   payload: #{ level: "info", msg: "refund approved" } },
});
```

Then a reviewer (via the Inbox UI, or another rule) resolves the item — and the **approval reactor**
closes the loop:

```rhai
inbox.resolve("refund-proposed", "approved");   // → reactor releases the held effect (held→pending)
// inbox.resolve("refund-proposed", "rejected"); // → reactor DISCARDS it (never delivered)
// inbox.resolve("refund-proposed", "deferred"); // → left held (inert in v1, re-resolve later)
```

The relay then delivers the released effect **exactly once**. Reuses the exact `Item` + `Resolution` +
outbox trio the coding workflow ships — no new approval primitive.

- **Two writes, effect-first.** `request_approval` stages the held effect FIRST, then records the item,
  so a `needs:approval` item never exists without its gated effect (a mid-verb deny leaves at most a
  harmless held effect). Both charge the per-run write budget; both are `caller ∩ grant` gated
  (`mcp:outbox.enqueue:call` for the effect, `mcp:inbox.record:call` for the item).
- **The request is caller-gated; the release is a system transition.** The requester needs the outbox
  stage cap; the *release* runs under the reactor's host authority, gated only by the resolution
  existing — never by re-checking a user cap. So there is **no user verb to force a release**, and a
  released effect can never exceed what was staged. The reviewer sees the exact held effect before
  approving (informed consent).

## 6. Returning chart data — a rule IS a panel source (rules-for-widgets)

A rule's **`output`** (the last expression) is what a dashboard/widget draws when it binds a
`{tool:"rules.run", args:{rule_id}}` source. The convention: **the last expression is an array of row
maps**; a `time` column makes it a time-series. `RuleOutput::Scalar(array)` and `RuleOutput::Grid`
(a returned `Grid`) both render — the host unwraps the envelope by shape. (Findings/`log` do **not**
render as rows — they're the insights/inbox plane's food; a panel draws `output` only.)

You don't have to reverse-engineer the shape. The **chart-return helpers** (`chart` family, pure compute
over rows, zero authority) make it one line:

```rhai
// One line per building → a multi-line chart. `wide` pivots long rows to one column per series.
let rows = query("demo-buildings",
  "SELECT s.name AS building, substr(pr.time,1,10) AS ts, ROUND(SUM(pr.value),0) AS kwh
   FROM point_reading pr JOIN point p ON p.id = pr.point_id
   JOIN meter m ON m.id = p.meter_id JOIN site s ON s.id = m.site_id
   WHERE p.name = 'Energy kWh' GROUP BY s.name, ts").records();
wide(rows, "ts", "building", "kwh")
```

| Helper | Shape |
|---|---|
| `timeseries(rows, "ts")` | Normalize `ts` (ISO-8601 string \| epoch-secs \| epoch-ms → canonical epoch-ms), rename it `time`, sort ascending. The frame builder tags the x-axis without guessing. |
| `timeseries(rows, "ts", ["v1","v2"])` | Same, plus trim to `time` + the named value columns. |
| `wide(rows, "ts", "series", "value")` | Long→wide pivot: one row per timestamp, one numeric column per distinct `series` (multi-line shape). |
| `category(rows, "name", "value")` | Bar/pie shape: one label column + one numeric column (validated). |

Raw rows keep working — the helpers are normalization sugar, not a required layer.
`timeseries(query(…).records(), "ts")` as the last line is a complete chart-ready rule. A missing/
non-numeric column is a clear **author error** (surfaced verbatim in the run result), never a silent
empty chart.

`.records()` returns maps keyed by the SELECT aliases on **every** source kind — federation (sqlite/
postgres, whose wire shape is column-aligned arrays) collapses to maps at the cage seam, so the same
one-liner works against `demo-buildings` (federation) and a Surreal source alike. The catalog has a
worked `category(...)` example: `buildings-intensity-chart` (a bar-chart of kWh per m² per building);
bind a panel to `{tool:"rules.run", args:{rule_id:"buildings-intensity-chart"}}` and it renders.

**Heavy rules + fast refresh don't mix.** A rule re-runs per panel per refresh (bounded by `RuleLimits`
+ the `viz.query` timeout, but **not** by frequency). A big `frame()`/slow federation query on a 5 s
auto-refresh multiplies — same exposure as a heavy SQL source. Keep expensive rules on slow refreshes.

**Panel runs are read-only.** The picker emits `route:false` on a rule source, so a repainting dashboard
does **not** stamp a new inbox item + must-deliver outbox entry on every refresh from an `alert()` in the
body — the finding still returns in the result, it just doesn't fan out. A scheduled flow
(`rules.eval`, default `route:true`) still alerts: one rule, two consumption modes.

## 7. Raising an insight — a durable fault record from the rule body (rule-raises-insight)

A threshold rule that notices a fault can record it as a **durable, deduped insight** in one line —
no flow, no `insight` sink node. The `insight` handle is the **rule producer door** onto the insight
plane, riding the same generic MCP seam as `inbox`/`outbox`/`channel`:

```rhai
let hot = history("series", "cooler.temp", "24h").filter("value > 5.0");
if hot.size() > 0 {
    let id = insight.raise(#{
        dedup_key: "cooler-temp-high",   // stable identity — re-raising the same key upserts, never duplicates
        severity: "warning",             // info | warning | critical
        title: "Cooler temp high",
        body: #{ series: "cooler.temp", value: 9.1 },   // free-form evidence
        tags: #{ area: "hvac" },         // facets for the matcher / Insights page filters
    });
    // …and when the same rule later sees a human behind the run, or the fault clear:
    insight.ack(id);            // open → acked   (stop paging, keep the record)
    insight.close(id);          // * → resolved   ("close" maps to the insight.resolve verb)
    insight.close(id, "manual");// optional note on close
}
```

- `insight.raise(#{…}) -> id` returns the insight's stable id so you can `ack`/`close` it later **in
  the same body**. You may omit `origin` — the cage defaults it to `{ kind:"rule", ref:<rule id> }`
  (the rule *is* the origin). `producer`/`acked_by`/`resolved_by` are **host-forced from the
  principal** — a rule can't forge an actor even by putting one in the map.
- `insight.close(id)` is the author-facing name for the **`insight.resolve`** verb/cap (grepping for
  `resolve`? `close` is its cage name).
- **Caps:** raise needs `mcp:insight.raise:call`, ack `mcp:insight.ack:call`, close
  `mcp:insight.resolve:call` — each re-checked mid-run under `caller ∩ grant`. **No new capability**
  and **no new MCP verb**: this is a cage door onto the three verbs that already ship.
- **Metered:** each raise/ack/close is a charged write (like `channel.post`), so an insight-storm loop
  trips `max_writes` — the same DoS bound.

### `emit` vs `alert` vs `insight.raise` — which door?

| You want… | Use | Lives where | Lifespan |
|---|---|---|---|
| A value in the run's **result** (a number, a row, a note the caller reads back) | `emit(#{…})` / `log(…)` | The `findings` this run returns | Ephemeral — gone after the run |
| To **route attention now** (an inbox item + a must-deliver outbox notification) | `alert(#{…})` | Inbox + outbox (motion), subject to `route:false` | The inbox item's lifecycle |
| A **durable, queryable, deduped** cross-cutting fault record with severity + occurrence history + a lifecycle | `insight.raise(#{…})` | The `insight:{ws}:{id}` record + occurrence ring (state) | open → acked → resolved, deduped on `dedup_key` |

The one-liner: **`emit` is what this run found; `alert` is "someone look now"; `insight.raise` is
"this fault exists in the world until closed".** They compose — a rule can `emit` a summary *and*
`insight.raise` the underlying fault.

### `route:false` panel runs raise nothing

A panel repaint runs the rule `route:false` (see §6). On such a run **every `insight` method is a
no-op**: it charges nothing, writes nothing, logs an honest `insight.<verb> skipped: read-only panel
run` line, and `raise` returns an echoed id so the body doesn't error. Rationale: an `insight.raise`
is a *stronger* effect than `alert()` — it writes a durable record **and** fans out the notify ladder.
If `alert()` is suppressed on a repaint, raising an insight (the heavier effect) must be too — otherwise
a dashboard viewed by ten people, refreshing every 30 s, would inflate the insight's `count` and
re-fire notifications purely from *viewing*. Dedup collapses the *record* to one, but not the
count-bump / occurrence append / notify re-fire — so the whole call is skipped, not just deduped. A
scheduled flow (`rules.eval`, default `route:true`) raises normally: one rule, two consumption modes.

## 8. Long-running runs — jobs, checkpoints, pause/resume (long-running-rules)

A batch-shaped rule (sweep a month of series, classify thousands of rows through `ai.*`) runs as a
**durable background job**: `rules.run_async {body|rule_id, params, ts?}` → `{run_id}` immediately.
The run gets its own governor profile (10 min wall-clock / 100× ops by default — `LB_RULES_JOB_*`
knobs), survives observation, and is **pausable, resumable (even across a node restart), and
cancellable**:

```bash
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"rules.run_async","args":{"rule_id":"monthly-sweep"}}'
# → {"run_id":"01J..."}   then observe / control:
#   rules.runs.get {run_id}      → {status, live, progress:{pct,msg}, checkpoints, result?, error?, tail}
#   rules.runs.suspend {run_id}  → parks within one bytecode op (status → suspended)
#   rules.runs.resume {run_id}   → replays the body over its checkpoints
#   rules.runs.cancel {run_id}   → terminal from any non-final state; re-cancel is a no-op
```

Inside the cage, the **`job` handle** (present in every run; ephemeral in a sync `rules.run`, so one
body works in both modes) is how a long rule reports progress and makes itself resumable:

```rhai
let days = job.step("plan", || make_day_list(param("month")));   // memoized unit of work
let mut done = 0;
for d in days {
    if job.should_stop() { break; }                  // tidy early return on pause/cancel
    job.step(`day:${d}`, || scan_one_day(d));        // a resumed run replays this as a LOOKUP
    done += 1;
    job.progress(done * 100 / days.len(), `day ${d} done`);
}
```

- **Resume = replay over checkpoints, never a VM snapshot.** The body re-runs from the top;
  `job.step`/`job.set` values persisted to the job transcript short-circuit, and every messaging
  write replays onto its original deterministic id (the pinned `ts` + write ordinal) — an upsert,
  never a duplicate. Wrap anything expensive (an `ai.*` call, a big collect) in `job.step` so a
  resume never re-spends it.
- **Pause bites without author cooperation** — the per-operation governor observes the control flag;
  `should_stop()` is just the polite fast path. A native call in flight (a collect, a completion)
  finishes first; the abort lands at the next op.
- **Budgets:** 256 durable checkpoints/run (author error past it), 1000 durable progress beats
  (advisory — dropped past it, the run continues).
- **Caps:** one per verb (`mcp:rules.run_async:call`, `mcp:rules.runs.get|list|suspend|resume|cancel:call`);
  read ≠ control, so an observer role holds get/list only. A resumed run executes under the
  **resumer's** `caller ∩ grant` — orphaned runs (`live:false` after a crash) wait for a caller,
  never auto-resume under stored authority.

## 9. Working with data — the stdlib (time, json, stats, mathx, frames)

Once rows are in hand, the cage carries a full **data standard library** — pure, deterministic,
zero-authority compute (`docs/scope/rules/data-stdlib-scope.md`). The authoritative surface is
`rules.help` (every function, family, signature, description); the map:

- **`time`** — the run's injected logical clock (never a wall-clock; rhai's `timestamp()` is
  disabled): `time.now()/now_ms()`, `iso`/`date`/`clock`/`format(ts,fmt[,"+10:00"])`, `parse`/
  `parse_fmt`, components (`year`…`weekday_name`, `iso_week`), boundaries (`start_of_day/week/
  month/year`, `end_of_day/month`), arithmetic (`add/sub(ts,"7d")`, `floor/ceil(ts,"15m")`,
  `diff`, `since`/`until`, `ago`). Plus duration helpers: `dur_secs("24h")`, `dur_human(n)`,
  `seconds(n)`…`weeks(n)`.
- **`json_*` / shape helpers** — `parse_json`/`to_json[_pretty]`, deep paths (`jget(v,"a.b[0].c"
  [,default])`/`jset`/`jhas`), `merge` (RFC-7386-ish, `()` deletes), `flatten`/`unflatten`,
  `pick`/`omit`, `entries`/`from_entries`, row ops (`pluck`, `index_by`, `group_rows`, `where_eq`,
  `sort_by`, `uniq_by`, `count_by`, `rows_epoch`), and the SurrealDB normalizers (`thing_id`/
  `thing_tbl`, `epoch` — ISO string | secs | ms → secs).
- **`stats`** — over plain arrays: mean/median/mode/variance/std_dev/sem, percentile/quantiles/iqr,
  skewness/kurtosis/histogram, zscores/minmax_scale/rank, corr/spearman/cov/linreg/predict/
  forecast_linear, cumsum/diffs/pct_changes/shift_arr, dropna/fillna/ffill/bfill/interp_linear,
  outliers_iqr/outliers_z/is_anomaly, top_k/argmax, sample/shuffle (**seed mandatory** —
  determinism), rolling_mean/sum/min/max/std + ema. Missing-value policy: `()`/non-numeric/NaN are
  skipped by aggregations, preserved positionally by windowed fns.
- **`mathx`** — `round_to`, `trunc_to`, `sign`, `clamp`, `lerp`, `map_range`, `pct`, `pct_change`,
  `safe_div(a,b,default)`, `log_base`, `hypot`, `approx_eq`.
- **`Frame`** — polars in the cage: `g.frame()` materializes a Grid (same gated seam as
  `g.records()`, capped at `max_frame_rows`) and `frame(records)` builds one from row maps; then
  select/drop/rename/sort/unique/filter_*/fill_null*/group_agg/join/vstack/pivot/melt/rolling_*/
  ewm_mean/diff/cumsum/zscore/bucket(ts,"15m")… plus **`f.sql("SELECT … FROM self")`** (in-memory
  only — no file/net table ever registered) and exits `f.records()`, `f.col("v")` (feeds the stats
  fns), `to_csv_string()`/`to_json_string()`. Join/vstack/pivot OUTPUTS are capped too
  (`max_frame_cells`) — a cross-join explosion is an author error, not a node stall.

**When to use which:** push down when big (the lazy `Grid` composes SQL at the source), compute
locally when shaped (a bounded `Frame`/array). Don't materialize a million rows to average them —
`g.col("v").avg()` pushes the mean down; `g.frame()` is for reshaping the bounded result.

## Gotchas

- **A run can't widen its invoker** — a data verb hitting a source the *caller* lacks is denied
  mid-run under `caller ∩ grant`, even with a valid body. Hold the target caps too, not just `rules.run`.
- **`rules.run` is synchronous and bounded** — governors cap a single run. Anything long/retrying/
  restart-surviving is a **chain** = an `lb-jobs` job (`rule-chains-scope.md`), not a blocking loop in
  a handler.
- **`save` never executes** — saving persists the body; only `run` evaluates it.
- **`id` defaults to `name`** and upserts in place (no version history).
- **The script sees no secrets** — no provider key (AI-gateway holds it; the rule adapter passes the
  resolved `ModelAccess`, never a key), no DB DSN (`federation` holds it). Same posture for both seams.
- **`ai.*` errors when unconfigured, never fabricates** — a workspace with no selected model (or a node
  with no provider) gets the honest `"AI not configured for rules"` error, not a made-up answer; the
  rest of the rule (data reads, `emit`) runs regardless.
- **Workspace wall** — ws-B can neither `get`/`run` a ws-A saved rule nor read a ws-A source;
  `ai.ask` schema introspection sees only the workspace's own sources.
- **Denials are opaque** — a blocked `source`, a tripped budget, and a rejected `eval` all surface as
  opaque errors before anything runs.
- **A held effect is never delivered until approved** — `request_approval`'s `on_approve` is staged
  `held`, so the relay skips it; nothing fires on the mere *request*. It fires only after
  `inbox.resolve(id, "approved")` and the reactor's release. A rule resolving its *own* request is
  possible (if the caller holds `inbox.resolve`) but is a foot-gun, not the intent — human sign-off is.
- **`insight.close` is `insight.resolve`** — the author-facing name differs from the verb/cap. Hold
  `mcp:insight.resolve:call` (not `…close…`) to close from a rule.
- **A re-open still charges** — a `route:true` re-run at the same `dedup_key` charges the meter even
  when the raise only bumps `count` (no new record). The meter counts write *attempts*, not new rows.
- **`insight.raise` is produce-only from a rule** — a rule *produces* insights; it does not browse them
  (no `insight.get`/`list` in the cage). Query the data plane (`source("store")`) if a rule must branch
  on an existing insight's state.

## Related

- Scope: `docs/scope/rules/rules-engine-scope.md`; the DAG that chains rules on `lb-jobs`:
  `docs/scope/rules/rule-chains-scope.md`; the `ai.*`→real-model binding:
  `docs/scope/rules/rules-ai-wiring-scope.md`.
- The catalog pick a rule's `ai.*` honors: `docs/scope/agent/agent-catalog-scope.md` (`agent.config`).
- The external-source engine `source("…")` reaches: `docs/skills/datasources/SKILL.md`
  (`federation.query`) — and the SurrealDB-never-a-DataFusion-source hard line. To seed + register
  the `demo-buildings` source these examples use (Docker-free): `docs/testing/datasources/README.md`
  Step 0 (`make seed-demo-sqlite`).
- Platform series a rule reads: `docs/skills/ingest-series/SKILL.md` (`series.*`).
- `alert` targets: `docs/skills/channels-inbox-outbox/SKILL.md` (inbox/outbox).
- Propose-and-approve loop: `docs/scope/rules/rules-approvals-scope.md` (the `needs:approval` item +
  held effect + release reactor); the messaging verbs it builds on: `rules-messaging-scope.md`.
- README §3 (rules 1/2/4/5/6/7), §6.5 (MCP), §6.7 (AI-gateway), §6.9 (jobs), §6.10 (inbox/outbox).
  Source: `rust/crates/host/src/rules/`; ported from `rust/rubix-cube/rbx-server/src/rules/`
  (MIT/Apache-2.0). Routes: `rust/role/gateway/src/server.rs`.
