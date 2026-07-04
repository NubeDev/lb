# Rules scope — the data stdlib: time, JSON, stats, and polars frames in the script cage

Status: scope (the ask). Promotes to `public/rules/rules.md` once shipped (the whole surface lives
inside the one rhai cage — rules + the flows `rhai` node — so it documents there, not under datasources).

A rule (and a flow `rhai` function node — same cage) can *fetch* data from every gated source —
`source("series")`, `source("<datasource>")`, `source("query:<id>")` — but once the rows are in hand
the author has almost nothing to *work* with them: no timestamp (``channel.post("ops", #{ body:
`posted at ${???}` })`` has no answer today), no date math or formatting, no median/percentile/rolling
window outside SQL pushdown, and no ergonomic way to reshape the JSON shapes SurrealDB and federated
sources actually return (record ids like `sensor:abc123`, ISO-8601 datetime strings, nested maps).
This scope adds the missing **data standard library** to the one rhai cage: a `time` handle driven by
the run's injected logical clock, `json`/shape helpers for real SurrealDB output, a broad
scalar/array **stats** family, and a **polars-backed `Frame`** for real dataframe compute over
already-collected rows — all pure, deterministic, zero-I/O compute that adds **no new authority**.

> Read with: `rules-engine-scope.md` (the cage + `DataSeam` + the lazy Grid this computes
> over), `rules-messaging-scope.md` (the logical `now` + write-determinism contract this
> must not break), `../datasources/datasources-scope.md` (the federation extension whose rows land here),
> `../flows/` (the `rhai` function node routes through host `rules.eval` — the same cage, so it gets
> this library for free), `../../FILE-LAYOUT.md` (one verb family per file), README §3 (rules 1/4/5/9).

---

## Goals

- **A `time` scope handle** closed over the run's injected logical clock (`now`, unix **seconds** —
  the same value every `channel.post`/`inbox.record` already stamps as `ts`): current time, parsing,
  formatting, components, arithmetic, bucketing. No wall-clock anywhere — determinism and re-run
  idempotency hold exactly as `rules-messaging-scope.md` requires.
- **JSON + SurrealDB-shape helpers**: parse/stringify, deep path get/set, merge/flatten, and the
  unglamorous-but-daily verbs for rows as they *actually* return — split a record `Thing` id, turn an
  ISO datetime string into epoch seconds, pluck/index/group an array of row maps.
- **A scalar + array stats family** (data-science 101 without leaving the body): median, mode,
  variance, percentiles, z-scores, correlation, linear regression, rolling windows, EMA, outlier
  detection, histograms, fill/interpolate — over plain rhai arrays, so it works on any rows however
  they were fetched.
- **`Frame` — polars in the cage** (<https://github.com/pola-rs/polars>): `g.frame()` materializes a
  Grid (through the existing gated seam) into an in-memory polars `DataFrame`; `frame(records)`
  builds one from an array of maps. The frame gets the full local-compute surface — select/filter/
  sort/group/join/pivot/rolling/describe — **plus `f.sql("SELECT … FROM self")`** via polars' SQL
  context, so one method covers the long tail. Local compute only: a Frame never opens a connection,
  reads a file, or reaches a source the seam didn't already allow.
- **One cage, every caller.** The library registers in `lb-rules`' `verbs::register`, so rules,
  `rules.eval`, and the flows `rhai` function node (which routes through `rules.eval`) all get it —
  no second registration path, no drift.
- **~150+ functions** organized per FILE-LAYOUT: one verb family per file, never a `utils.rs`.

## Non-goals

- **No new data access.** Nothing here fetches: the only ways rows enter remain `source(...)` /
  `query(...)` / the messaging reads — all seam-gated. `Frame` is post-collect compute. A polars
  `scan_csv`/`read_parquet`/cloud reader is **never** registered (the cage stays zero-I/O).
- **Not a second query engine beside federation.** DataFusion in the `federation` extension pushes SQL
  *to sources* (datasources-scope). Polars here computes *over rows already in the run*. The split
  stands: pushdown for volume, Frame for shape/stats on the bounded result.
- **No wall-clock, no ambient randomness.** `time.*` reads the injected clock only; rhai's built-in
  `timestamp()` (a live `Instant`) is **disabled** alongside `eval`; `sample`/`shuffle` require an
  explicit seed. A re-run with the same inputs and `now` stays byte-identical.
- **No plotting/rendering.** Charts are the dashboard/viz plane's job; a rule returns data
  (`f.records()`, a histogram array), the UI draws it.
- **No timezone database in v1.** UTC everywhere, plus an explicit fixed-offset parameter
  (`time.format(ts, fmt, "+10:00")`). A full IANA tz table is additive (open question).
- **No breaking of the existing Grid surface.** The lazy SQL-pushdown Grid verbs stay the primary
  path for big data; `frame()` is the explicit "now compute locally" step with its own row cap.

## Intent / approach

**Three layers, one cage.** (1) *Pure verb families* — `time`, `json`, `stats`, `mathx` — land in
`lb-rules/src/verbs/` as sibling files to `channel.rs`/`timeseries.rs`, registered by the existing
`verbs::register`. They are closures over nothing but the run's pinned `now`; no seam, no cap, no
I/O, so the cage's security model ("absence of capability + presence of limits") is untouched.
(2) The **`time` handle** is pushed into scope exactly like `ai`/`inbox`/`outbox`/`channel` —
the author's mental model stays "handles for platform surfaces" (`time` is the run's clock surface).
(3) **`lb-frame`** is a **new crate** (`rust/crates/frame/`) linking polars + rhai, exposing one
`register(engine, &FrameLimits)`; `lb-rules` consumes it behind a default-on `frames` cargo feature.
The new crate keeps `lb-rules`' "links only rhai + serde" doctrine legible — the heavy dependency is
one clearly-bounded crate, feature-severable if a target ever can't carry it.

**Why polars and not DataFusion in the cage.** We already ship DataFusion — in the federation
*extension*, where SQL pushes down to external sources (datasources-scope). Pulling DataFusion into
the rule cage would blur the two roles rule 3 keeps apart and buy nothing the Grid's SQL pushdown
doesn't already do. Polars is the complement: an eager, columnar, in-memory engine with the exact
verb vocabulary data work needs (rolling, pivot, describe, join) and a small SQL front-end
(`SQLContext`) for the long tail — over rows that are already inside the run's budget. Rejected:
re-implementing 60 stats verbs by hand over `Vec<Value>` (slow, buggy, unmaintained); polars is the
industry answer and we treat it like we treat rhai — a vetted embedded library.

**Why chrono and not jiff.** The date math needs a library (hand-rolled calendars are a bug farm).
`chrono` is already in the workspace dependency graph (SurrealDB links it), so it adds zero new
supply-chain roots; we use only its **pure** computation (construct/format/parse/arithmetic) and ban
`chrono::Utc::now()`/`Local` in `lb-rules` via a workspace clippy `disallowed-methods` entry — the
clock is *injected*, always. Rejected: `jiff` (nicer API, but a new root dependency for the same
math); a hand-written epoch→civil converter (fine for `year()`, hopeless once `parse`/`format`/ISO
weeks arrive).

**Why a string-SQL method on Frame instead of an expression DSL.** Polars' native lazy expression
API doesn't survive a script boundary without inventing a DSL (rejected: a bespoke mini-language is
exactly the maintenance trap FILE-LAYOUT warns about). Method-per-verb covers the common 90%;
`f.sql("…FROM self")` (polars' own `SQLContext`, table registered as `self`) covers the rest with a
syntax every author already knows — and it parses/executes entirely in-memory, so the cage's
zero-I/O posture holds.

**Determinism is a contract, not a hope.** The run's ids and outputs must be replay-stable
(rules-messaging-scope): therefore `time.now()` = the injected clock, rhai's `timestamp()` is
disabled in `build_engine` (one more `disable_symbol`, beside `eval`), and every stochastic verb
takes a mandatory `seed`. The testing plan pins this with a run-twice-byte-identical test.

## The function catalog

Names are final unless the build finds a collision; each family is one file. Functions marked ✱
already exist in rhai's standard package (`Engine::new()` registers it) — listed so the build **does
not duplicate them**, only documents them in the skill.

### `time` — the clock handle (`verbs/time.rs`, ~35 fns)

All timestamps are unix **seconds** (i64) unless suffixed `_ms`. All formatting is UTC unless an
explicit `"+HH:MM"` offset argument is given. `fmt` is strftime.

| Function | Returns |
|---|---|
| `time.now()` / `time.now_ms()` | the run's logical clock (secs / ms) |
| `time.iso(ts)` / `time.iso_ms(ts_ms)` | `"2026-07-04T03:21:00Z"` |
| `time.date(ts)` / `time.clock(ts)` | `"2026-07-04"` / `"03:21:00"` |
| `time.format(ts, fmt)` / `time.format(ts, fmt, offset)` | strftime, UTC or fixed offset |
| `time.parse(s)` | ISO-8601/RFC-3339 (or epoch-secs/-ms numerics-as-string) → secs |
| `time.parse_fmt(s, fmt)` | strptime → secs |
| `time.from_ymd(y, m, d)` / `time.from_parts(y, m, d, h, mi, s)` | build a ts |
| `time.year/month/day/hour/minute/second(ts)` | components |
| `time.weekday(ts)` / `time.weekday_name(ts)` | ISO 1=Mon…7=Sun / `"Friday"` |
| `time.day_of_year(ts)` / `time.iso_week(ts)` | ordinal / ISO week no. |
| `time.days_in_month(ts)` / `time.is_leap_year(ts)` / `time.is_weekend(ts)` | calendar predicates |
| `time.start_of_day/week/month/year(ts)` | floor to boundary |
| `time.end_of_day/month(ts)` | ceil-1s to boundary |
| `time.add(ts, "24h")` / `time.sub(ts, "7d")` | duration arithmetic (`s/m/h/d/w`) |
| `time.floor(ts, "15m")` / `time.ceil(ts, "1h")` | bucket alignment (matches `rollup` buckets) |
| `time.diff(a, b)` / `time.diff_days(a, b)` | signed difference (secs / whole days) |
| `time.since(ts)` / `time.until(ts)` | `now − ts` / `ts − now` (secs) |
| `time.ago(ts)` | `"3h 20m ago"` (humanized, for message bodies) |

### `dur_*` — durations (`verbs/duration.rs`, extends the existing file, ~8 fns)

| Function | Returns |
|---|---|
| `dur_secs("24h")` / `dur_ms("15m")` | parse the `s/m/h/d/w` form → number |
| `dur_human(secs)` | `"1d 2h 5m"` |
| `seconds(n)`, `minutes(n)`, `hours(n)`, `days(n)`, `weeks(n)` | constructors → secs |

### `json_*` + shape helpers (`verbs/json.rs`, ~25 fns)

| Function | Returns |
|---|---|
| `parse_json(s)` | string → map/array/scalar |
| `to_json(v)` / `to_json_pretty(v)` | any value → JSON string |
| `jget(v, "a.b[0].c")` / `jget(v, path, default)` | deep path get (never throws) |
| `jset(v, path, val)` / `jhas(v, path)` | deep set (returns new value) / predicate |
| `merge(a, b)` | deep merge (b wins) — RFC-7386-style, `()` deletes |
| `flatten(map, ".")` / `unflatten(map, ".")` | nest ↔ dotted keys |
| `pick(map, [keys])` / `omit(map, [keys])` | shape trimming |
| `entries(map)` / `from_entries(array)` | map ↔ `[[k, v], …]` |
| `pluck(rows, "field")` | array-of-maps → array of that field |
| `index_by(rows, "id")` / `group_rows(rows, "key")` | array → map / map-of-arrays |
| `where_eq(rows, key, val)` | filter rows by field equality |
| `sort_by(rows, key)` / `sort_by(rows, key, desc)` / `uniq_by(rows, key)` | row ops |
| `count_by(rows, key)` | `#{ val: n, … }` frequency map |
| `thing_id("sensor:abc")` / `thing_tbl("sensor:abc")` | split a SurrealDB record id |
| `epoch(v)` | ISO string \| epoch-secs \| epoch-ms → secs (the "whatever the source returned" normalizer) |
| `rows_epoch(rows, "ts")` | normalize a ts column across an array of rows |

(✱ rhai already ships `keys`, `values`, `contains`, `filter`/`map`/`reduce` with closures, `split`,
`trim`, `to_upper`, `replace`, `sub_string`, and friends — documented in the skill, not re-added.)

### `mathx` — scalar extras (`verbs/mathx.rs`, ~12 fns)

✱ rhai ships `abs/floor/ceil/round/sqrt/exp/ln/log/sin/cos/tan/min/max/pow`. We add:

`round_to(x, dp)`, `trunc_to(x, dp)`, `sign(x)`, `clamp(x, lo, hi)`, `lerp(a, b, t)`,
`map_range(x, in_lo, in_hi, out_lo, out_hi)`, `pct(part, whole)`, `pct_change(from, to)`,
`safe_div(a, b, default)`, `log_base(x, b)`, `hypot(a, b)`, `approx_eq(a, b, eps)`.

### `stats` — array statistics (`verbs/stats.rs` + `verbs/window.rs`, ~40 fns)

All take a plain rhai array of numbers (non-numeric/`()` entries: see `dropna`/`fillna`); windowed
fns return an array of the input length (leading `()` until the window fills).

| Family | Functions |
|---|---|
| Center/spread | `sum`, `mean`, `median`, `mode`, `min_of`, `max_of`, `range_of`, `variance`, `std_dev`, `sem` |
| Quantiles | `percentile(a, p)`, `quantiles(a, [ps])`, `iqr(a)` |
| Shape | `skewness(a)`, `kurtosis(a)`, `histogram(a, bins)` → `[#{lo, hi, n}, …]` |
| Normalize | `zscores(a)`, `minmax_scale(a)`, `clip_arr(a, lo, hi)`, `rank(a)` |
| Relate | `corr(a, b)` (Pearson), `spearman(a, b)`, `cov(a, b)`, `linreg(xs, ys)` → `#{slope, intercept, r2}`, `predict(model, x)`, `forecast_linear(a, n)` |
| Sequence | `cumsum(a)`, `cummax(a)`, `cummin(a)`, `diffs(a)`, `pct_changes(a)`, `shift_arr(a, n)` |
| Windows | `rolling_mean/sum/min/max/std(a, w)`, `ema(a, alpha)` |
| Missing | `dropna(a)`, `fillna(a, v)`, `ffill(a)`, `bfill(a)`, `interp_linear(a)` |
| Outliers | `outliers_iqr(a, k)` / `outliers_z(a, thr)` → indices; `is_anomaly(a, x, thr)` |
| Selection | `top_k(a, k)`, `bottom_k(a, k)`, `argmax(a)`, `argmin(a)`, `sample(a, n, seed)`, `shuffle(a, seed)` |

### `Frame` — polars (`crates/frame/`, ~60 methods)

Construction: `g.frame()` (materialize a Grid through the seam, capped at `max_frame_rows`);
`frame(records)` (array of maps); `f.records()` / `f.to_grid_json()` back out; `f.col("value")` →
plain array (feeds every `stats` fn above).

| Family | Methods |
|---|---|
| Inspect | `shape`, `height`, `width`, `columns`, `dtypes`, `head(n)`, `tail(n)`, `slice(o, n)`, `describe()`, `null_count()`, `is_empty` |
| Shape | `select([cols])`, `drop([cols])`, `rename(from, to)`, `with_col_from(name, array)`, `sort(col)`, `sort(col, desc)`, `unique()`, `unique_by([cols])`, `reverse()` |
| Filter | `filter_eq/ne/gt/ge/lt/le(col, v)`, `filter_in(col, [vs])`, `filter_between(col, lo, hi)`, `filter_null/not_null(col)`, `sample(n, seed)` |
| Missing | `drop_nulls()`, `drop_nulls([cols])`, `fill_null(v)`, `fill_null_strategy(col, "forward"\|"backward"\|"mean"\|"zero")` |
| Aggregate | `mean/median/sum/min/max/std/var/quantile(col, …)`, `count()`, `n_unique(col)`, `value_counts(col)` |
| Group/join | `group_agg([keys], #{ col: "agg", … })`, `join(other, on, "inner"\|"left"\|"outer"\|"anti")`, `vstack(other)`, `pivot(idx, cols, vals, agg)`, `melt([ids], [vals])` |
| Series ops | `rolling_mean/sum/min/max/std(col, w)`, `ewm_mean(col, alpha)`, `diff(col)`, `pct_change(col)`, `cumsum(col)`, `shift(col, n)`, `rank(col)`, `zscore(col)`, `clip(col, lo, hi)` |
| Time | `bucket(ts_col, "15m")` (epoch-aware truncate — pairs with `time.floor`) |
| SQL | `f.sql("SELECT series, avg(value) v FROM self GROUP BY series")` — polars `SQLContext`, in-memory only |
| Export | `to_csv_string()`, `to_json_string()` (bounded by the engine string cap — feeds `channel.post` bodies) |

Total: **~180 functions** across six files + one crate — comfortably past the ask, every one earning
its place (no `misc`).

## How it fits the core

- **Tenancy / isolation:** nothing new to isolate — the library adds **zero data access**. Rows
  enter only through the existing seams (workspace-pinned, caps-checked); `g.frame()` materializes
  via the same `DataSeam::collect` the Grid already uses. The isolation tests assert the *absence*:
  a Frame/stat/time call can be made with an empty allowlist and an empty cap set.
- **Capabilities:** none required, none added. Pure compute is below the capability line (like `+`
  or `array.map` today); authority stays at the seams. The deny path is unchanged and untouched.
- **Placement:** `either` — symmetric by construction; it's library code inside the cage. No
  `if cloud`.
- **MCP surface (§6.1):** **N/A — deliberately.** No new verbs: the whole feature lives *inside*
  `rules.run`/`rules.eval` bodies. CRUD/list/feed/batch don't apply to an in-cage stdlib; long
  compute is bounded by the run governors, and anything longer is (as ever) a job/flow.
- **Data (SurrealDB):** no new tables, no reads, no writes. State vs motion untouched.
- **Bus (Zenoh):** untouched.
- **Sync / authority:** N/A — no durable state; a Frame dies with the run (rule 4 holds).
- **Secrets:** none.
- **One responsibility per file:** `verbs/time.rs`, `verbs/json.rs`, `verbs/stats.rs`,
  `verbs/window.rs`, `verbs/mathx.rs` in `lb-rules`; `crates/frame/src/` as folder-of-verbs
  (`construct.rs`, `filter.rs`, `group.rs`, `window.rs`, `sql.rs`, `export.rs`, `limits.rs`), each
  ≤400 lines. **Never** a `utils.rs`.
- **SDK/WIT impact:** none — internal crates only; no wasm/native ABI change. The flows `rhai` node
  inherits everything through `rules.eval` with zero flows-side change.
- **Governors (the real security work here):** the cage's `on_progress` deadline **cannot interrupt
  a native polars call** — so the bound moves to the *inputs*: a new `max_frame_rows` (default
  200 000 rows) + `max_frame_cells` (default 2 000 000) in `RuleLimits` is enforced at `frame()`/
  `frame(records)`/`vstack`/`join` output, and `to_csv_string`/`to_json_string` respect the existing
  `max_string_bytes`. Rhai's built-in `timestamp()` is disabled in `build_engine` (determinism).
- **Skill doc:** yes — this is author-drivable surface. The build **extends
  `docs/skills/rules/SKILL.md`** with a "working with data" chapter (time/json/stats/frames, each
  example grounded in a live Playground run) rather than coining a new skill — rule authors find it
  where they already look.

## Example flow

A facilities analyst computes a daily anomaly report without leaving the rule body:

```rhai
// 1. Fetch (gated, pushdown as today) — last 7 days of cooler temps.
let g = source("series")
          .filter(`series == 'cooler.temp' && ts > ${time.sub(time.now(), "7d")}`);

// 2. Materialize locally (capped) and compute — polars, in the cage.
let f = g.frame()
         .sort("ts")
         .with_col_from("smooth", rolling_mean(g.frame().col("value"), 12))
         .bucket("ts", "1h");
let daily = f.sql("SELECT series, avg(value) AS v FROM self GROUP BY series");

// 3. Stats over a plain column.
let temps = f.col("value");
let bad = outliers_z(temps, 3.0);
let fit = linreg(f.col("ts"), temps);          // #{slope, intercept, r2}

// 4. Shape + report — deterministic clock, humanized formatting.
if bad.len() > 0 {
    channel.post("ops", #{ body:
      `⚠ ${bad.len()} anomalies this week (trend ${round_to(fit.slope * 86400.0, 3)}/day). `
      + `p95=${percentile(temps, 95)}  report ${time.date(time.now())} ${time.clock(time.now())}Z` });
    alert(#{ severity: "warn", body: to_json(daily.records()) });
}
```

Every fetch went through the existing gates; everything after was pure compute; a re-run with the
same inputs and `ts` produces byte-identical output (and the same `channel.post` id → upsert).

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — real engine, real seams, no mocks:

- **Capability-deny (§2.1) — inverted here:** prove **no new authority**: a body using only
  `time`/`stats`/`frame(records)` runs green with an **empty cap set and empty allowlist**; and a
  Frame gives no back-door — `g.frame()` on a disallowed source is denied exactly as `g.records()`
  is (same seam, same opaque deny).
- **Workspace-isolation (§2.2):** unchanged paths, re-asserted through `frame()`: a ws-B rule
  cannot materialize a ws-A source into a Frame (the pin refuses at collect, before polars sees a row).
- **Determinism (the contract):** one body exercising `time.*`, `sample(seed)`, and a Frame
  pipeline, run twice with the same `now` and inputs → **byte-identical** output and identical write
  ids; `timestamp()` is proven disabled (eval error).
- **Governors:** `frame()` past `max_frame_rows` aborts with a clear author error (no partial
  frame); `to_csv_string` past `max_string_bytes` aborts; a `while(true)` around frame ops still
  dies on the operation governor between calls.
- **Unit, per family:** each file ships its table-driven tests (time components across DST-irrelevant
  UTC edges — leap years, month ends, ISO week 53; `epoch()` across the three input shapes; stats
  against known fixtures incl. NaN/null policies; every Frame method on a seeded fixture frame;
  `f.sql` happy + syntax-error paths surface verbatim as author feedback).
- **Integration (real node):** a Playground rule end-to-end — seed real series records, fetch,
  `frame()`, stats, `channel.post` the report — asserted over the real gateway; and a **flows**
  `rhai` node using `time.iso`/`rolling_mean` to prove the one-cage claim.
- **Regression home:** anything that breaks logs under `docs/debugging/rules/` (the cage owns the
  behavior; datasources only feeds it).

## Risks & hard problems

- **Polars is a heavy dependency.** Build time and binary size grow; with the zig-cc linker setup
  this must be proven early (first task of the build: compile `lb-frame` with the minimal feature
  set — `lazy`, `sql`, `rolling_window`, `pivot`, `strings`, `temporal` — and measure). Mitigation:
  the `frames` cargo feature severs it cleanly if a target can't carry it; the rest of the stdlib
  has no such weight.
- **Uninterruptible native calls.** A pathological polars op (a huge cross join) ignores the rhai
  deadline. Input caps (`max_frame_rows`/`max_frame_cells`, enforced on *outputs* of join/vstack/
  pivot too) are the honest bound — the tests must include a join that would explode and prove the
  cap fires first.
- **`f.sql` is a wide surface.** It's in-memory-only by construction (no table providers registered
  beyond `self`), but the build must assert polars' SQL context cannot reach registration of
  external scans, and pin the polars version (a minor bump adding I/O functions to the SQL namespace
  would widen the cage silently — the version is a security pin, reviewed on upgrade).
- **API sprawl / duplication.** 180 functions invite drift (three ways to compute a mean: SQL
  pushdown, `stats::mean`, `f.mean`). That's intentional layering (push down when big, compute
  locally when shaped), but the skill doc must say *when to use which* or authors will materialize
  a million rows to average them.
- **NaN/null policy consistency.** SurrealDB returns `NONE`, federated sources return SQL `NULL`,
  rhai has `()`, polars has `null` *and* `NaN`. One documented rule everywhere: missing = `()` ↔
  `null`; `NaN` is normalized to null at the frame boundary; stats fns skip nulls and say so.
  Getting this wrong quietly corrupts results — it gets its own fixture tests.

## Resolved decisions

- **Placement of polars: a new `lb-frame` crate, consumed by `lb-rules` behind a default-on
  `frames` feature.** Keeps the cage crate's lean-deps doctrine legible and the heavy dep severable.
  Rejected: polars directly in `lb-rules` (muddies the "links only rhai" contract); a host-side
  registration hook (would put engine-shape knowledge in the host — wrong seam).
- **Date library: chrono, pure computation only.** Already in the workspace graph via SurrealDB;
  wall-clock constructors banned by clippy `disallowed-methods`. Rejected: jiff (new root for the
  same math), hand-rolled calendars (bug farm).
- **Clock semantics: unix seconds, UTC, from the injected `now` only** — the same clock the
  messaging ids already use. `_ms` variants for interop; rhai `timestamp()` disabled.
- **The long tail goes through `f.sql(…)`, not a bespoke expression DSL.** Polars' own SQLContext,
  `self`-only. Rejected: a mini expression language (a maintenance trap and a parser to secure).
- **Stochastic verbs take a mandatory seed.** No default-seeded RNG anywhere in the cage.

## Open questions

- **Polars feature set + version pin — RESOLVED (Phase 0):** exact minimal feature list that covers
  the catalog is the 17-feature curated set in `crates/frame/Cargo.toml` (`lazy`, `sql`,
  `rolling_window`, `pivot`, `strings`, `temporal`, `dtype-full`, `json`, `describe`, `is_in`,
  `round_series`, `cum_agg`, `rank`, `diff`, `pct_change`, `ewma`, `zip_with`), `default-features =
  false`, version pinned `=0.54.4`. `sql`+`lazy` DO pull `polars-io`'s csv/parquet/cloud crates
  transitively, but **runtime-proven unreachable from the SQL namespace** (`read_csv`/`read_parquet`
  are not registered as table functions — see the security probe in `crates/frame/src/lib.rs` tests),
  so no further feature disabling is needed.
- **`max_frame_rows` default — RESOLVED (Phase 0, recommendation taken):** 200k rows / 2M cells is
  the default carried in `FrameLimits`; calibration against the Playground on dev hardware + the
  `env::rules::MAX_FRAME_ROWS` wiring lands in Phase 2/3.
- **Timezone table:** is fixed-offset formatting enough for v1 message bodies, or do dashboards need
  IANA names (`"Australia/Sydney"`) rule-side? If yes, `chrono-tz` is the additive path.
- **Does `interpolate`/`gapfill` on the Grid (today identity plans, see `verbs/timeseries.rs`) get
  re-pointed at the Frame** (`g.frame().fill_null_strategy(...)`) so the v1 stubs become real, or
  stay SQL-side? Recommendation: re-point — local interpolation over a bounded frame is exactly
  what polars is for.

## Related

- `rules-engine-scope.md` — the cage, governors, Grid, and seams this extends.
- `rules-messaging-scope.md` — the logical-`now`/determinism contract `time` must honor.
- `../datasources/datasources-scope.md` — federation/DataFusion pushdown (the *fetch* half; this is the *compute* half).
- `../flows/` — the `rhai` function node (via `rules.eval`) that inherits the library.
- `../../FILE-LAYOUT.md` — folder-of-verbs; the catalog's file map follows it.
- `../../skills/rules/SKILL.md` — the skill the build extends with the "working with data" chapter.
- README §3 rules 1 (symmetric), 4 (stateless), 5 (capability-first — held by *adding no authority*),
  9 (no mocks — fixtures are real seeded records).
- <https://github.com/pola-rs/polars>, <https://rhai.rs> — the two embedded engines.
