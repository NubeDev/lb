# Jobs

The SurrealDB-native durable job record + the raw `lb-jobs` verbs that persist and resume it
(README §6.9). A job is **state** (in the store, addressed `job:{id}` within a workspace); the
central agent's resumable session lives here. TODO: the full queue/worker/retry/scheduling model.

## Drain scan + retention (shipped 2026-07-03)

The reactors (channel-agent 2 s, flow 5 s) tick a **drain scan** — "which jobs of this kind are
still resumable?" — and terminal rows accumulate forever from routine traffic. Left unbounded, both
became an O(table) CPU bomb on a long-lived node (a full core burned just re-scanning the `job`
table; see `debugging/jobs/node-pegs-cpu-reactor-rescans-job-table.md`). Two composable fixes:

### Indexed drain scan — `pending` is O(pending), not O(table)

`lb_jobs::pending(store, ws, kind)` runs a single indexed query:

```sql
SELECT data FROM job WHERE data.kind = $kind AND data.status IN ['running','suspended']
```

backed by `DEFINE INDEX job_kind_status ON TABLE job COLUMNS data.kind, data.status`. The field path
is `data.kind`/`data.status` because every store write nests the host body under a `data` wrapper —
the index must target the stored path or SurrealDB silently falls back to a scan. The index is
ensured **lazily, per-namespace, idempotent** (`define_job_index`, called on first `create`), the
same first-touch pattern prefs/tags use — there is no global boot-time schema pass. Cost now tracks
the *pending* count (a handful), not table size. It is **strictly safer** than the paged full-walk it
replaced on the "a late-sorting pending job is still found" property: there are no pages to fall off.

### Bounded retention — terminal rows stop growing

Count-bounded retention per workspace, default **500** kept per table (a generous window so ordinary
history isn't lost — bounding runaway growth, not aggressive GC). The delete predicate is
`status IN (terminal)` **and nothing else** — the one unacceptable failure is trimming a resumable
(`Running`/`Suspended`) job/run, which would double-run it, so a live row outside the window is kept
forever by design.

- **`job`** (`lb_jobs::retain_terminal`) — trims `done`/`failed`/`cancelled` jobs to the newest cap.
- **`flow_run` + `flow_step_output`** (`flows::retain_runs::retain_runs`) — trims finished runs
  (`success`/`partialFailure`/`failed`/`cancelled`; never `pending`/`suspended`) to the newest cap
  **and deletes each purged run's step rows in tandem** (the step table is ~2× the runs — the real
  disk bulk).

Both reuse `store/capped.rs`'s safe-delete idiom (`LET $keep = (SELECT … LIMIT n); DELETE … NOT IN
$keep`), workspace-walled via `query_ws`. They are **raw node-internal verbs** — no user-facing MCP
surface, no capability (the reactor holds its own `node:reactor` authority). Retention runs on the
flow reactor tick, throttled to every 30th tick (`flows::retention_sweep`), and fires on the first
tick so a freshly-booted node reclaims a bloated store immediately.

Config is a **compiled caller-owned default** (`DEFAULT_TERMINAL_JOB_CAP`, `DEFAULT_FINISHED_RUN_CAP`)
per `capped.rs`'s "defaults live in the caller". It is **not** a prefs key: prefs here is a closed
typed-axis system with no numeric key→value getter, so there is nowhere to resolve a retention number
from today; an operator override would slot in at the constant.

### Placement rationale (why a sweep, not a write-time trim)

`capped.rs` prefers an insert+trim in one transaction where there is a single write chokepoint. Here
neither table qualifies: `job` reaches terminal through **two** verbs (`complete`, `cancel`); and
while `flow_run` has a single terminal chokepoint (`set_run_status`), its `flow_step_output` rows are
keyed `{run_id}:{node}` and written by a different verb, so a trim at the run transition can't reach
the step bulk — they must be purged in tandem, keyed by the purged run ids, which only a sweep does.

### Dev relief

`make purge-store` wipes the dev node store (`.lazybones/data/dev-store`) — no rebuild, keys and
extensions untouched — for a dev box that already bloated before the fix landed.
