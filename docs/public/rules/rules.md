# Rules (public)

Status: **SHIPPED** (2026-06-28). Scope: `../../scope/rules/rules-engine-scope.md` (the `lb-rules`
engine) + `../../scope/rules/rule-chains-scope.md` (the DAG over `lb-jobs`). Session:
`../../sessions/rules/rules-session.md`. Source/attribution: ported from `rubix-cube`
(`rust/rubix-cube/rbx-server/src/rules/`, MIT/Apache-2.0) — see the session doc + `crates/rules`.

A workspace-authored, **sandboxed rules engine** (`lb-rules`) — an embedded `rhai` cage + a lazy,
column-oriented `Grid` + a verb library — ported from `rubix-cube` and **re-seamed onto the lazybones
chokepoints**: data through `store.query`/`series.*` (platform) or `federation.query` (external),
`ai.*` through the AI-gateway (metered + nsql-fenced), `alert` through the inbox + outbox. Plus **rule
chains**, a DAG of saved rules driven over `lb-jobs` + a SurrealDB run-store. All reached the same
way — workspace-first, capability-gated MCP verbs (rule 7).

## The cage (the security model, in-process)

A rule runs in a fresh `rhai` engine with **zero I/O surface** and every resource governor set
(`max_operations`, `max_call_levels`, string/array/map caps, a wall-clock deadline; `eval` disabled,
modules disabled). A rule can do nothing but call the verbs the host hands it — and **every verb runs
the host `caps::check`**. Defense in depth: the governors bound work + time (DoS); `caps` bounds
authority (rule 5). `lb-rules` links only `rhai` + `serde` — no DataFusion, no store, no socket.

## The verb library

- **Data:** `source(name)` (the uniform entry — platform vs federation chosen host-side), `query`,
  `history`, `span`/`last`, `param`.
- **Grid (lazy, column-oriented):** `filter`, `select`, `add_col`, `rename`, `group_by`+`agg`, `join`,
  `head`, `size`, `columns`, `records`; `Col` reductions `max`/`min`/`avg`/`sum`/`count`/`std`/`first`/
  `last`/`p`. Nothing scans until a reduction / `records` / a returned grid.
- **Timeseries (dialect-aware plan-builders):** `rollup`, `lag`, `delta`, `rate`, `interpolate`,
  `gapfill`, `resample`.
- **AI (gateway-seamed):** `ai.complete`, `ai.ask` (proposes SQL, **re-validated through the same gate
  a hand-written query takes — the fence**), `ai.classify`, `ai.embed`. A per-run `AiMeter` caps calls
  + summed tokens.
- **Emit:** `emit(map)` (a finding), `alert(map)` (a finding + an inbox item + an outbox notification),
  `log(msg)`.

A run returns `{output: scalar|grid|findings|nothing, findings, log, ms, ai}`.

## MCP surface — `rules.*`

| Verb | Cap | Does |
|---|---|---|
| `rules.run {body\|rule_id, params}` | `mcp:rules.run:call` | run ad-hoc or a saved rule → `{output, findings, log, ms, ai}`. Bounded by the governors → synchronous. |
| `rules.save {id, name, body, params}` | `mcp:rules.save:call` | upsert `rule:{ws}:{id}` (idempotent). |
| `rules.get {id}` | `mcp:rules.get:call` | read one saved rule (workspace-walled). |
| `rules.list {filter?}` | `mcp:rules.list:call` | list saved rules. |
| `rules.delete {id}` | `mcp:rules.delete:call` | tombstone (idempotent). |

Inside a run every data verb hits `caps::check` for the underlying `store.query`/`series.*`/
`federation.query` under `caller ∩ grant` — a rule cannot read a source its invoker lacks. The deny is
opaque.

## MCP surface — `chains.*` (the DAG)

| Verb | Cap | Does |
|---|---|---|
| `chains.save {…DAG}` | `mcp:chains.save:call` | validate the DAG (cycle/dangling/dup/self-edge/size — **rejected before any run**) + upsert `chain:{ws}:{id}`. |
| `chains.run {chain_id, params}` | `mcp:chains.run:call` | start a durable run → `{run_id}` (a chain is a **job**). |
| `chains.resume {chain_id, run_id}` | `mcp:chains.run:call` | re-drive an interrupted run (exactly-once). |
| `chains.get {id}` / `chains.list` | `mcp:chains.get`/`list:call` | read the DAG(s). |
| `chains.runs.get {chain_id, run_id}` | `mcp:chains.get:call` | the per-step DAG-canvas snapshot (status + outcomes). |
| `chains.delete {id}` | `mcp:chains.delete:call` | tombstone. |

A step runs one saved rule under `caller ∩ grant`; bindings are whole-value `${params.x}` /
`${steps.x.output}` / `${steps.x.findings}` references. `FailurePolicy::Halt` prunes a failed step's
subtree (run = PartialFailure); `Continue` releases dependents with the failed output as `null`. Run
state is durable (`chain_run` + per-step `chain_step` rows with a CAS claim), so a node restart +
`chains.resume` finishes the un-run steps **exactly once** (a redelivered step no-ops). Triggers:
Manual (`chains.run`) today; Cron (the S6 reactor) + Event (`bus.watch`) are the next additive slice.

## Data shape (SurrealDB, one datastore)

`rule:{ws}:{id}` (body + params), `chain:{ws}:{id}` (the DAG), `chain_run:{ws}:{run_id}` (lifecycle),
`chain_step:{ws}:{run_id}:{step_id}` (per-step claim + outcome + output). All workspace-walled.

## Tests (the gate — all green)

28 `lb-rules` unit tests (cage / grid plan-builders / **AI fence + budget** / DAG validation + binding
resolution) + 12 host integration tests (6 rules + 6 chains): capability-deny per verb, mid-run source
deny, workspace-isolation, the full seed-real-series → rollup+alert → inbox round-trip, AI budget, DAG
validation at save, diamond frontier order, Halt subtree-skip, and **restart-resume-exactly-once**. No
mocks — real store/caps/MCP host/lb-jobs; the only fake is the model provider behind the AI seam.
