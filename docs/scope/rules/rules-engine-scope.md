# Rules scope — `lb-rules`, an embedded rules/processing engine over the platform

Status: scope (the ask). Promotes to `public/rules/rules.md` once shipped.

We want a **rules engine** a workspace can author and run: a small, sandboxed script that reads
data, transforms it (more than just SQL — branch, join, reduce, classify, call AI), and **emits
findings/alerts** — exposed as MCP verbs so the UI, the AI agent, and other extensions all drive it
the same way. This is the lazybones-native version of the **`rubix-cube` rules engine** (an embedded
`rhai` cage + a lazy DataFusion `Grid` + a verb library), **ported onto our chokepoints**: its data
verbs read through our `series.*`/`data.query` MCP tools, its `ai.*` goes through the AI-gateway, its
`emit/alert` raise inbox items, and (the sibling scope) its workflow DAG runs on `lb-jobs`. The
engine is an **in-process library**, not an external service — that is the whole reason it fits.

> Read with: `rule-chains-scope.md` (the DAG that chains rules on `lb-jobs`),
> `../datasources/datasources-scope.md` (the `federation` extension a rule reaches for MySQL/Timescale),
> `../ai-gateway/ai-gateway-scope.md` (the model seam `ai.*` uses), `../jobs/jobs-scope.md`,
> `../inbox-outbox/outbox-scope.md`, `../auth-caps/auth-caps-scope.md`, README §3 (rules), §6.5 (MCP).

---

## Source: ported from `rubix-cube`, not copied

The reference is the working engine at `rust/rubix-cube/rbx-server/src/rules/` (read 2026-06-28). We
take its **architecture and most of its verb surface verbatim**, and rewire exactly three seams to
the lazybones core. What transfers nearly unchanged, what changes, and what is dropped:

| `rubix-cube` piece | Disposition in `lb-rules` |
|---|---|
| `sandbox.rs` — `RuleLimits` + `build_engine` (governors: `max_operations`/`max_call_levels`/string/array/map caps, `disable_symbol("eval")`, `set_max_modules(0)`, wall-clock `on_progress` deadline) | **Lift verbatim.** The cage is "absence of capability + presence of limits" — exactly our capability-first posture, in-process. |
| `grid.rs` — the lazy, column-oriented `Grid`/`Col`/`GroupedGrid`/`Span` (compose SQL plans; `filter`/`select`/`add_col`/`rename`/`group_by`/`join`/`col` reductions; no row iterator) | **Lift, re-target.** The composition logic is unchanged; the *collect* path calls our data verbs (below), not a local DataFusion engine. |
| `verbs/data.rs` — `query`/`dataset`/`history`/`span`/`last`/`param` | **Lift, re-seam.** `source(...)`/`query(...)` resolve to **`data.query`/`series.*` MCP verbs** (platform data) or **`federation.query`** (external sources), not a local engine snapshot. |
| `verbs/timeseries.rs` — `rollup`/`align`/`interpolate`/`gapfill`/`resample`/`lag`/`delta`/`rate` | **Lift verbatim** (pure plan-builders over a `(ts,value)` grid). |
| `verbs/emit.rs` — `emit`/`alert`/`log` into shared `Collectors` | **Lift, re-seam.** Findings still collect per-run; `alert` now hands off to **`inbox`** (and must-deliver notifications to the **outbox**), not a TODO marker. |
| `verbs/ai.rs` + `ai.rs` — `ai.ask`/`complete`/`classify`/`embed`, the `AiMeter` budget, the re-validate-the-proposed-SQL **fence** | **Lift, re-seam.** The seam points at the **AI-gateway** (§6.7), not `LLM_PROVIDER`+`reqwest`. The budget meter and the nsql fence are kept exactly. |
| `runtime.rs` — `Rule`/`RuleRun`/`RuleOutput`(scalar/grid/findings/nothing)/`Finding`/`LogLine` | **Lift, re-key.** `project_id` → `workspace`; `allowed_datasets` → the workspace's granted sources. |
| `routes.rs` — actix `POST /rules/run` | **Replace.** Surface becomes **MCP verbs** (`rules.run`/`rules.save`/…), reached over the existing gateway/host, not a bespoke HTTP route. |
| `workflow/**` — the DAG coordinator/model/run_store/context | **Moves to `rule-chains-scope.md`** — ported onto `lb-jobs` (its `JobQueue`/`WorkflowRunStore`/cron seams swap for `lb-jobs`/SurrealDB/the reactor). |

The license is shared (MIT/Apache-2.0, same repo lineage). Record the attribution in the implementing
session and a crate-level comment, per `../agent-run/agent-run-scope.md`'s "Source review" precedent.

## Goals

- A **`lb-rules` core crate**: a sandboxed `rhai` engine with the `rubix-cube` governors, a verb
  library, and the `Grid` value — evaluating a saved or ad-hoc rule and returning a typed
  `RuleOutput` (scalar | grid | findings | nothing) plus its findings/log.
- The **data verbs read through MCP**, not a private engine: platform data via `data.query`/`series.*`
  (SurrealDB, native, authoritative); external data via `federation.query` (the `datasources`
  extension). One uniform `source(...)` surface to the rule author, two correct paths beneath.
- The **`ai.*` namespace** routed through the AI-gateway, keeping `rubix-cube`'s two invariants: the
  per-run **budget meter** (call + token caps) and the **re-validation fence** (a model-proposed query
  is re-checked through the same gate a hand-written one is, before it ever runs).
- **`emit`/`alert`** that raise real **inbox** items (attention) and route must-deliver notifications
  through the **outbox** — not a sandbox-internal marker.
- An **MCP surface** (`rules.*`) so the UI, the AI agent, and other extensions author/run rules under
  the same workspace-first, capability-gated chokepoint — and a **Playground** page to write + run.
- Saved rules persisted as **SurrealDB records**, workspace-scoped (the one datastore).

## Non-goals

- **Chaining rules into a DAG** — that is `rule-chains-scope.md` (it reuses `lb-jobs`). This scope is
  the single-rule engine + its MCP surface; the chain scope builds on it.
- **Owning external DB connections.** A rule never opens a socket. MySQL/Timescale live behind the
  `federation` native extension (`../datasources/datasources-scope.md`); the rule reaches them only
  through `federation.query`.
- **A second model runtime.** `ai.*` calls the existing AI-gateway; we do **not** embed an LLM server.
- **A new scripting ABI for extensions.** `lb-rules` is a host capability exposed as MCP verbs, not a
  parallel extension runtime competing with wasm/native (rule 1 & 7). Rhai is the *rule-author's*
  language, sandboxed and capability-free; it is never an extension-loading mechanism.
- **Embedding DataFusion in core.** The heavy federation engine stays in the `datasources` extension.
  `lb-rules` itself pulls only `rhai` (+ serde) — platform data is queried via MCP, where SurrealDB
  already does aggregation/graph/vector.

## Intent / approach

**The cage is the security model, in-process.** `rubix-cube`'s `build_engine` registers *no* file/
net/process API and sets every resource governor; a rule can do nothing but call the verbs we hand it.
That is precisely capability-first (rule 5) realized as an embedded sandbox: the Rhai layer can't
reach the store, the bus, or a socket — only the verbs, and every verb runs the host's `caps::check`.
Defense in depth: the governors bound *work and time* (DoS), `caps::check` bounds *authority*.

**One `source(...)` surface, two correct paths — this is the load-bearing design choice.** To the rule
author, `source("cooler.temp")` and `source("timescale")` read alike. Underneath:

- a **platform** source resolves to `data.query`/`series.*` → **SurrealDB**, native and authoritative.
  We do **not** route platform data through DataFusion; SurrealDB already does aggregation/graph/vector,
  and routing it through a federation engine would fork the authority and bypass `caps` (rule 2/5).
- an **external** source resolves to `federation.query` → the **`datasources` extension** → MySQL/
  Timescale, `net:*`-gated. The `Grid` composition (`filter`/`rollup`/`join`/…) is identical; only the
  base-grid producer differs.

The `Grid` stays *lazy* (it carries composed query text + a context, materializing only on `collect`/
a `Col` reduction/`records`/return) so method chaining never copies data — `rubix-cube`'s exact model.
The one behavioral change: `Grid::collect` calls an MCP verb instead of a local engine. A platform
grid collects via `data.query`; a federation grid via `federation.query`. The validator/allowlist that
`rubix-cube` re-runs on every collect becomes **the host `caps::check` + workspace pin** on every verb
call — same property ("a rule can read no source a direct query in the same workspace couldn't"),
enforced at the real chokepoint.

**`ai.*` keeps its two invariants, re-seamed to the gateway.** The provider seam (`AiBackend` →
`propose_sql`/`complete`/`embed`) re-points at the AI-gateway (`ModelAccess`), so the rule never sees a
key and spend is metered where the platform already meters it. The **budget meter** (`AiMeter`:
per-run call + token caps, a `for`-loop of `ai.complete` can't run up an unbounded bill) and the
**nsql fence** (`ai.ask`'s proposed SQL is re-validated through the same gate `query()` uses before
execution — "no path from nsql to the engine skips the validator") transfer verbatim; they are exactly
the guards that make a model-driven query safe.

**Rejected — adopting Cube/Spice as a service.** Considered (and discussed at length pre-scope): wiring
Cube (semantic layer/BI) or Spice (federation engine) as an external service the platform points at.
Rejected: both are *services with their own API surface + security context*, which collides with rule 7
(MCP is the universal contract) and rules 5/6 (we'd trust their tenancy, not `caps`). `rubix-cube`
already proved the better path — embed the engine as a **library** (`rhai` + DataFusion-the-crate) and
front it with our own verbs. `lb-rules` embeds the *rule* engine; the `datasources` extension embeds
the *federation* engine; neither runs as a foreign service.

**Rejected — a YAML/JSON-only decision-table language (e.g. ZEN/GoRules).** Considered for ops-authored
declarative rules. Deferred, not adopted: Rhai already covers procedural + declarative needs, and a
second authoring language is surface we don't need yet. The verb library is the stable contract, so a
declarative front-end can be added later as *another caller of the same verbs* without re-work.

## How it fits the core

- **Tenancy / isolation:** a `RuleRun` is bound to one `workspace` (from the calling token, host-set,
  never script-set). Every data verb resolves the workspace's granted sources; `ai.ask` schema
  introspection sees only the workspace's own sources. ws-B can neither run a ws-A saved rule nor read
  a ws-A source. Proven across store + MCP (mandatory isolation test).
- **Capabilities:** the verbs are the gate. `rules.run`/`rules.save`/`rules.get`/`rules.list`/
  `rules.delete` each gated `mcp:rules.<verb>:call`. Inside a run, every data verb call hits the host
  `caps::check` for the underlying `data.query`/`series.*`/`federation.query` cap under
  `caller ∩ grant` — a rule cannot widen beyond what its invoker holds. `ai.*` requires the agent/
  gateway cap. The deny is opaque. Every grant has a deny test.
- **Placement:** `either` (symmetric). `lb-rules` is a pure in-process library; it runs wherever the
  node runs. No `if cloud`. (A rule that reaches an external source depends on the `datasources`
  extension being installed there — a placement property of *that* extension, not of `lb-rules`.)
- **MCP surface (§6.1 — judged, not defaulted):**
  - **Run (the core add):** `rules.run {body|rule_id, params}` → `{output, findings, log, ms}` — ad-hoc
    (Playground) or by saved id. The hot path; bounded by the governors, so it stays a synchronous
    call (a long/batch rule belongs in a **chain** → a job, per `rule-chains-scope.md`).
  - **CRUD:** `rules.save` (create/update a saved rule: name, body, declared params), `rules.delete`.
    Each its own verb + cap (FILE-LAYOUT one-verb-per-file).
  - **Get / list:** `rules.get {id}`, `rules.list {filter?}` — workspace-scoped reads.
  - **Live feed:** N/A for a single rule run (bounded, returns its result). The *chain* run streams
    progress (`rule-chains-scope.md`); a future per-run `RunEvent` projection can reuse `agent-run`'s
    vocabulary, deferred here.
  - **Batch:** N/A here — a bulk/long evaluation is a **chain**, which is a **job** (§6.10). Stated so
    no one grows a blocking N-item loop in a `rules.run` handler.
- **Data (SurrealDB):** saved rules are records — `rule:{ws}:{id}` (name, `body` Rhai source, declared
  `params`), workspace-walled. No new persistence layer; a typed table behind MCP. Findings/log are
  returned from a run (and, for a saved rule run inside a chain, recorded on the chain's job
  transcript — `rule-chains-scope.md`). A rule reads data through verbs; it never writes platform state
  except via a write-shaped verb that is itself gated.
- **Bus (Zenoh):** none directly from a single run. `alert` hands a must-deliver notification to the
  **outbox** (durable), not raw pub/sub; an `emit` that should surface live in a channel uses the
  existing `bus.publish` verb under its cap. State vs motion held (§3 rule 3).
- **Sync / authority:** a saved rule is authoritative on its hosting node like any record; a run is
  node-local and stateless (the cage holds no durable state — rule 4). Durable, resumable evaluation is
  the **chain** (a job), covered in the sibling scope.
- **Secrets:** none in `lb-rules`. The script never sees a provider key (the AI-gateway holds it) nor a
  DB DSN (the `datasources` extension holds it) — same posture as `rubix-cube` hiding both behind seams.
- **SDK/WIT impact:** none. `lb-rules` is a host crate + MCP verbs; it does not touch the wasm/native
  ABI. (The verb library is an internal Rust surface, versioned as normal code.)

## Example flow

A facilities analyst writes a food-safety rule in the Playground.

1. They type a Rhai body and hit **Run**. The UI calls `rules.run {body, params}` over MCP; the host
   authorizes `mcp:rules.run:call` workspace-first, builds a `RuleRun` bound to `acme`.
2. `lb-rules` builds a fresh sandboxed engine (governors set, zero I/O surface) and registers the verb
   library closing over the workspace's granted sources + a fresh `Collectors` + `AiMeter`.
3. The body runs on a blocking thread:
   ```
   let hot = source("cooler.temp").last("24h")     // -> series.* (SurrealDB), ws-pinned
       .rollup("1h", "max")
       .filter("max > 5.0");
   if hot.size() > 0 {
       let note = ai.complete("Summarize the food-safety risk", hot);  // -> AI-gateway, metered
       alert(#{ level: "critical", series: "cooler.temp", msg: note });
   }
   ```
   `source`/`rollup`/`filter`/`size` compose a lazy grid and collect via `series.*`; `ai.complete`
   bounds the grid to the context cap and charges the budget meter; `alert` appends a finding marked
   for routing.
4. The run returns `{output: {kind:"findings"}, findings:[…], log:[…], ms}`. The Playground shows the
   result grid / findings / log inline. The `alert` finding is handed to the **inbox** (and, if the
   rule is a saved/scheduled one, a must-deliver notification goes to the **outbox**).
5. **Deny path:** the analyst tries `source("payroll_db")` without the `federation.query`/source grant
   → the verb returns an opaque error before any query runs. A `for`-loop of `ai.complete` trips the
   per-run budget and aborts with a budget error. A script `import`/`eval` is rejected by the cage.
6. They click **Save** → `rules.save {name:"cooler-foodsafety", body, params}` persists
   `rule:acme:cooler-foodsafety`. ws-B cannot see or run it.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — the gate. **No mocks**: real store, real
caps, real MCP host; data is **seeded as real records** and read through the real verbs. The **only**
sanctioned fake is the model provider behind the AI-gateway seam (a true external, already stubbed at
that boundary, `ai-gateway` §3) — used to exercise the `ai.*` fence/budget without a live model.

- **Capability-deny (§2.1):** `rules.run`/`rules.save`/`rules.delete` denied without their cap; a rule
  whose data verb hits a source the **invoker** lacks is denied mid-run (the `caller ∩ grant`
  intersection) — even though the rule body is valid. `ai.*` denied without the gateway cap.
- **The cage holds (port `rubix-cube`'s sandbox tests):** `eval`/`import` rejected; `max_operations`
  trips an infinite loop fast; the wall-clock deadline aborts a slow run; oversized string/array/map
  rejected. These are DoS-boundary unit tests with tight `RuleLimits`.
- **The AI fence + budget (port verbatim):** a **malicious-LLM stub** proposing `SELECT … FROM
  <foreign/blocked>` via `ai.ask` is **re-validated and rejected** before execution (no nsql path
  skips the gate); `AiMeter` caps calls and summed tokens (a loop can't overspend); the rejected call
  isn't counted.
- **Workspace-isolation (§2.2):** ws-B cannot `rules.get`/`run` a ws-A saved rule; a ws-B run cannot
  read a ws-A source; `ai.ask` schema introspection never lists a ws-A source — across store + MCP.
- **Offline / sync (§2.3):** a saved rule survives a node restart (it's a record); a single `rules.run`
  is node-local and needs no sync. (Durable resumable evaluation is tested in `rule-chains-scope.md`.)
- **Unit:** the `Grid` plan-builders (filter/select/group_by/join/rollup/align/locf/lag/delta/rate)
  produce the expected composed query and the right columns; `Col` reductions (max/min/avg/sum/count/
  std/first/last/p); `duration_to_interval` parsing; `RuleOutput` classification (scalar/grid/findings/
  nothing); injected ids/clock (determinism — no wall-clock in results).
- **Integration (real gateway/UI):** a Playground `*.gateway.test.tsx` runs a rule end-to-end against a
  real spawned node — seed real series, run a rollup+alert rule, assert the findings + the inbox item.

## Resolved decisions

No open questions — these are the long-term answers the build follows.

- **Language → Rhai (port `rubix-cube`'s cage verbatim).** Sandboxed, governed, capability-free,
  `sync`-feature for `Send+Sync` verb closures. A declarative decision-table front-end (ZEN/GoRules) is
  **deferred, not rejected** — it would be another caller of the same verb library, additive, so not
  built now. Lua/Rune considered and dropped: Rhai is what's proven here and integrates cleanly.
- **Data access → through MCP verbs, never a private engine.** Platform data via `data.query`/
  `series.*` (SurrealDB, authoritative); external via `federation.query` (the extension). `lb-rules`
  does **not** link DataFusion. Rationale: SurrealDB already does aggregation/graph/vector for platform
  data, and keeping the heavy engine out of core preserves single-artifact symmetric nodes (rule 1).
- **`Grid` stays lazy + column-oriented (port verbatim), but collects via a verb.** Keep the deliberate
  absence of a per-row iterator (users stay vectorized; `head(n)` bounded inspection + `Col`
  reductions). The only change is `collect` calls an MCP verb. The `(ts,value)` timeseries verbs move
  unchanged.
- **`ai.*` → the AI-gateway seam, keep the fence + the budget.** Re-point `AiBackend` at `ModelAccess`;
  the nsql re-validation fence and the per-run `AiMeter` transfer exactly. The script never holds a key.
- **`alert` → inbox + outbox (resolve `rubix-cube`'s stage-03 TODO now).** `emit` collects a finding;
  `alert` additionally raises an **inbox** item and routes must-deliver notification through the
  **outbox** (durable, at-least-once, dedup) — never raw pub/sub, never a sandbox-internal send.
- **MCP surface, not an HTTP route.** Replace `rubix-cube`'s actix `POST /rules/run` with `rules.*` MCP
  verbs over the existing host/gateway — rule 7. The Playground calls the verbs, not a bespoke route.
- **Persistence → saved rules are SurrealDB records (`rule:{ws}:{id}`), one datastore.** Body is Rhai
  source; declared params are a typed list. No new store; a table behind MCP.
- **Per-run limits + budgets → config, workspace-scoped defaults.** The `RuleLimits` governors and the
  `AiMeter` caps come from node config (the `env::rules::*` knobs `rubix-cube` uses), with room for a
  per-workspace override record later (additive, not v1).
- **A single `rules.run` is synchronous and bounded; long/batch work is a chain (a job).** The governors
  bound a single run; anything that should run long, retry, or survive a restart is expressed as a
  **chain** (`rule-chains-scope.md`), which is an `lb-jobs` job. No blocking N-item loop in a handler.

## Related

- `rule-chains-scope.md` — the DAG that chains saved rules, ported onto `lb-jobs` (the workflow half of
  `rubix-cube`).
- `../datasources/datasources-scope.md` — the `federation` native extension a rule reaches for external
  sources via `federation.query`; the SurrealDB-native-vs-federated split.
- `../ai-gateway/ai-gateway-scope.md` — the `ModelAccess` seam `ai.*` re-points at; the budget/idempotency.
- `rules-ai-wiring-scope.md` — wires the `ai.*` model seam to the real agent (retires the hardcoded
  `DisabledModel` on the `rules.run` bridge; resolves the workspace's agent-catalog model selection).
- `rules-messaging-scope.md` — the `inbox`/`outbox`/`channel` rhai handles: explicit, caller-gated
  CRUD on the messaging planes (generalizes this scope's `alert` → inbox/outbox routing).
- `data-stdlib-scope.md` — the data standard library *inside* this cage: the `time`
  handle (logical clock), JSON/SurrealDB-shape helpers, array stats, and the polars-backed `Frame`
  (`lb-frame`) for post-collect compute. Pure verbs, no new authority; extends `verbs::register`.
- `long-running-rules-scope.md` — job-backed rule runs (`rules.run_async` + `rules.runs.*`):
  the long-run governor profile, the in-cage `job` handle (checkpoints/progress/should_stop),
  and suspend/resume/cancel — supersedes this scope's "a long/batch rule belongs in a chain"
  pointer now that chains are retired to flows.
- `../jobs/jobs-scope.md`, `../inbox-outbox/outbox-scope.md` — the durable job the chain reuses; the
  inbox/outbox `alert` routes to.
- `../auth-caps/auth-caps-scope.md` — `caps::check`, the chokepoint every verb runs.
- `../../vision/0003-iot-dashboard.md` — the worked example this serves (threshold rules + alerts over
  series, with external-warehouse data brought in through the extension).
- README `§6.5` (MCP — the contract), `§6.7` (AI-gateway), `§6.9` (jobs), `§6.10` (inbox/outbox), `§3`
  (rules 1/2/4/5/6/7).
- Source: `rust/rubix-cube/rbx-server/src/rules/` (the engine ported here; MIT/Apache-2.0).
