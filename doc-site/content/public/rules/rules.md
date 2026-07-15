# Rules

The `lb-rules` engine public docs — the sandboxed Rhai cage over the platform's gated verbs.
Promoted from `docs/scope/rules/` + the session logs as slices ship. The authoritative in-cage
function reference is the **`rules.help`** verb (the `lb_rules::CATALOG` — name, family, signature,
description for every function); `docs/skills/rules/SKILL.md` is the authoring guide.

Shipped so far:

- The engine + cage + lazy `Grid`, the `ai.*` budget/fence, `emit`/`alert` routing, `rules.*` CRUD
  and `rules.run`/`rules.eval` (`rules-engine`, `rules-ai-wiring`, `rules-messaging`,
  `rules-approvals`, rules-for-widgets, rule-raises-insight — see the skill doc).
- **Long-running rule runs** — jobs, checkpoints, pause/resume (`long-running-rules`, 2026-07-15).
- **The data stdlib** — `time`, `json`, `stats`, `mathx`, and the polars `Frame`
  (`data-stdlib`, 2026-07-15).

## Long-running runs — `rules.run_async` + `rules.runs.*`

A batch-shaped rule runs as a **durable background job** instead of tripping the synchronous
governors: `rules.run_async {body|rule_id, params, ts?}` returns `{run_id}` immediately; the run is
an `lb-jobs` record (kind `rule-run`) driven by a detached worker under its own governor profile
(default 10 min wall-clock / 500 M ops — `LB_RULES_JOB_*` env knobs).

Observe and control (one cap per verb; read ≠ control):

| Verb | Does |
|---|---|
| `rules.runs.get {run_id}` | Status, `live` flag, latest progress beat, checkpoint keys, result/error when terminal, bounded transcript tail. |
| `rules.runs.list {status?, limit?}` | The workspace's runs, newest first, terminal rows included. |
| `rules.runs.suspend {run_id}` | Cooperative pause — bites within one bytecode op, parks the job `suspended`. |
| `rules.runs.resume {run_id}` | Replays the body over its persisted checkpoints (works after a node restart; runs under the **resumer's** caps). |
| `rules.runs.cancel {run_id}` | Terminal from any non-final state; re-cancel is a clean no-op. |

Inside the cage, the **`job` handle** (present in every run; ephemeral in a sync `rules.run`) makes
a body resumable and observable:

```rhai
let days = job.step("plan", || make_day_list(param("month")));  // memoized unit of work
for d in days {
    if job.should_stop() { break; }
    job.step(`day:${d}`, || scan_one_day(d));   // a resume replays this as a LOOKUP — no re-spend
    job.progress(50, `day ${d} done`);
}
```

**Resume = replay over checkpoints, never a VM snapshot.** Checkpoints ride the `lb-jobs`
transcript (`Checkpoint`/`Progress` events, append-addressed); messaging writes replay onto their
original deterministic ids (the pinned `ts` + write ordinal) and upsert — exactly-once effects
without a queue. Budgets: 256 checkpoints/run (author error past it), 1000 durable progress beats
(advisory). Orphaned runs after a crash show `live:false` and wait for a caller's `resume` — never
an auto-resume under stored authority.

## The data stdlib — time, json, stats, mathx, frames

Pure, deterministic, zero-authority compute inside the cage (`data-stdlib-scope`): the run's
injected **`time`** handle (rhai's wall-clock `timestamp()` is disabled), **`json_*`/shape helpers**
for rows as sources actually return them (`thing_id`, `epoch`, `pluck`/`group_rows`/`sort_by`…),
a ~50-function **`stats`** family over plain arrays (median/percentiles/corr/linreg/outliers/
rolling/ema, `sample`/`shuffle` with a mandatory seed), **`mathx`** scalar extras, and the
**polars-backed `Frame`** (`g.frame()` / `frame(records)` → select/filter/group_agg/join/pivot/
rolling/`f.sql("… FROM self")`/exports), bounded by `max_frame_rows`/`max_frame_cells` — the caps
are enforced on construction *and* on join/vstack/pivot outputs, because the wall-clock governor
cannot interrupt a native polars call. No new authority anywhere: rows still enter only through the
gated `source(...)` seams, and `g.frame()` on a disallowed source denies exactly as `g.records()`
does.

Discovery: `rules.help` returns every function with its family/signature/description — the UI
autocomplete and agents read it; the skill doc's §9 is the human map.
