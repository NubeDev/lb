# Frontend scope — the rules workbench (Playground · datasources admin)

> **⚠ Phase 2 (the chain canvas) is RETIRED — the DAG story is now Flows.** The
> [`chains-retirement-scope`](../flows/chains-retirement-scope.md) deleted the `chains` engine
> outright (`flows` is the one DAG engine). The Phase-2 chain canvas below (`ChainCanvas`,
> `chains.*` routes, `chains.watch` follow-up) shipped, then was **removed** — its DAG capability is
> now the richer **Flows** canvas (`features/flows`, `flows.*`, live `flows.watch` SSE). The Playground
> (Phase 1, `rules.*`) and datasources admin (Phase 3, `datasource.*`) are **unchanged and stay**.
> Read the Phase-2 prose below as **history**; to author a rule-DAG, use Flows. See
> [`public/flows/flows.md`](../../public/flows/flows.md).

Status: **shipped** (Phases 1–3, 2026-06-29) → `public/frontend/rules-workbench.md`; built per
`sessions/frontend/rules-workbench-session.md`. (Originally: scope/the ask.) Target stage:
**S9+ collaboration UI** (builds directly on the shipped rules/chains/datasources plane —
`rules.*`/`chains.*`/`datasource.*` — and the shipped S9 real-session shell). This is the frontend
slice both `scope/rules/rules-engine-scope.md` ("a **Playground** page to write + run") and
`scope/rules/rule-chains-scope.md` ("the DAG canvas colours as steps settle") explicitly named as the
remaining work, plus the datasources admin page from `scope/datasources/datasources-scope.md`.

We want a **first-party rules workbench in the shell**: a logged-in user opens a workspace, writes and
runs Rhai **rules** in a Playground, wires saved rules into a **chain** on a visual DAG canvas and
watches each step settle, and (as an admin) registers **external datasources** a rule can reach.
Everything runs against a **real node** — real store, real caps, real Zenoh motion, the real
`rules.*`/`chains.*`/`datasource.*` host verbs that are **already shipped** — seeded with **real
records** through the real write path (no mocks; CLAUDE §9). This scope adds **no host work**: it is the
**gateway routes + the React surface** over verbs that exist, mirroring the dashboard surface
verb-for-verb (`scope/frontend/dashboard-scope.md`'s "core in the trusted shell, mirrored over the
gateway" pattern).

This scope is **three sequenced phases**. **Phase 1 (this doc's build-ready ask)** is the **Playground**
— write/run/save a single rule. **Phase 2** is the **chain canvas** (a React Flow DAG over `chains.*`).
**Phase 3** is the **datasources admin page** (over `datasource.*`). Phases 2–3 are roadmapped here with
their dependencies and de-risked decisions; only Phase 1 is specified to "code it with no open question."

---

## Goals

### Phase 1 — the Playground (the build-ready ask)

- **A rules workbench surface** (`ui/src/features/rules/`) — a cap-gated nav slot opening a two-pane
  page: a **left rail** of saved rules (`rules.list`) and an **editor pane** (a CodeMirror editor — the
  shipped `@uiw/react-codemirror` + `lang-javascript`, no new dep) where a user writes a Rhai rule body
  + declares params.
- **Run a rule** — a **Run** button calls `rules.run {body, params}` (ad-hoc) or `rules.run {rule_id,
  params}` (a saved rule) and renders the typed `RuleOutput` **three ways** by `kind`: a **scalar**
  card, a **grid** table (`columns`/`rows`), or a **findings** list (level-coloured, `alert`-marked);
  plus the **log** lines and a **budget readout** (`ms` + `ai` calls/tokens). One render component per
  output kind (FILE-LAYOUT).
- **Honest cage/deny states** — a denied source (`mcp:store.query`/`federation.query` the invoker
  lacks), an AI-budget-exceeded run, a syntax/cage error (`eval`/`import` rejected, an infinite loop
  tripped) each render a **clear, specific author-feedback state**, never a fake result. The deny is
  opaque where it should be (a missing tool cap) and explanatory where it's author feedback (a parse
  error) — exactly the host's `RulesError` → `ToolError` mapping (`BadInput` is shown; `Denied` is a
  generic "not permitted").
- **Save / get / delete** — **Save** calls `rules.save {id, name, body, params}` (idempotent UPSERT);
  the left rail's items open via `rules.get {id}` and delete via `rules.delete {id}` (tombstone). The
  *complete* CRUD surface, not a read-only subset (HOW-TO-CODE §3 step 4a).
- **The gateway mirror** — `rules.*` reachable from the browser over `role/gateway/src/routes/rules.rs`
  (the dashboard-route pattern): each route re-checks the cap server-side and takes ws+principal from
  the **token, not the body** (§7). The Tauri `invoke` path and the gateway route call the **same** host
  verb (one app, two deliveries — no `if cloud`).

### Phase 2 — the chain canvas (React Flow DAG) — roadmap, build-ready decisions taken

- **A visual DAG canvas** (`@xyflow/react`, already a dependency) rendering a saved chain (`chains.get`):
  **nodes = steps** (each naming a saved rule), **edges = `needs`**. Drag to add/connect; each node is a
  step card (id, rule, retry). Save/edit the DAG via `chains.save` — **validated up front** by the host
  (cycle/dangling/dup/self-edge/size), so an invalid edge renders an **inline error**, never a crash.
- **Run + settle colouring** — **Run** calls `chains.run {chain_id, params}` → `{run_id}`; the canvas
  then **colours each node as it settles** by polling `chains.runs.get {chain_id, run_id}` (the snapshot
  the run-store already rebuilds): `Pending → Running → ok | err | skipped`, with the **Halt-pruned
  subtree shown greyed** and the run status (`success`/`partialFailure`/`failed`) in a banner.
- **CRUD + gateway mirror** — `chains.save`/`get`/`list`/`delete`/`run`/`runs.get` over
  `role/gateway/src/routes/chains.rs`, same re-check-the-cap pattern.

### Phase 3 — the datasources admin page — roadmap, build-ready decisions taken

- **An admin page** (`ui/src/features/datasources/`, in the admin surface) over `datasource.*`:
  `datasource.list` (a roster — **never the DSN**, only kind + endpoint + a redacted secret ref),
  `datasource.add {name, kind, endpoint, dsn}`, `datasource.remove {name}`, and `datasource.test
  {source}` rendering a **real green/red connectivity probe**.
- **The `net:* + secret:*` approval shown at add time** — the Add form surfaces *which* grants the
  registration implies (`net:tls:host:port:connect`, `secret:federation/{name}:get`); the **approval is
  the install-grant record** the host enforces pre-connect, not a page concern. The page is the form +
  the honest display of "this is what you're approving."

---

## Non-goals

- **No host/backend changes.** `rules.*`/`chains.*`/`datasource.*` are **shipped** (sessions
  `rules/rules-session.md`, `datasources/{datasources,federation}-session.md`). This scope is gateway
  routes + the React surface only. (Phase 2's `chains.watch` SSE is the one *named* future host slice —
  deferred, see below.)
- **The federation extension ships NO UI, and must not.** The `federation` native (Tier-2) extension is
  a headless socket-/engine-owning OS sidecar (`tier="native"`, `[native]` exec, no `[ui]`/`[[widget]]`/
  `[[page]]`). The **datasources admin page is trusted first-party shell code driving the host
  `datasource.*` verbs over the gateway** — exactly as the dashboard surface drives `dashboard.*`,
  **never** something the extension contributes. Putting admin UI inside the least-trusted sidecar would
  invert the trust model. (This mirrors the dashboard scope's "dashboard core is trusted shell code;
  widgets are the cell-sized unit that federates" — here the *extension* is the headless unit, the
  *admin page* is the shell.)
- **No new editor / canvas dependency.** The Playground uses the shipped `@uiw/react-codemirror`; the
  chain canvas uses the shipped `@xyflow/react`. No Monaco, no new graph lib.
- **No `chains.watch` live SSE in Phase 2 (named follow-up).** Phase 2 colours the canvas by polling
  `chains.runs.get` (the durable snapshot) — Phase-1-honest, like the dashboard scope deferring the
  multiplexed series stream. The `chains.watch` SSE route (host + gateway) is the named scaling
  follow-up, not a silent gap.
- **No new sharing mechanism.** A saved rule/chain is already workspace-walled + cap-gated. Per-rule /
  per-chain *sharing* (private → team → workspace) is the named **Phase 1.5** follow-up, reusing the S4
  asset model exactly as the dashboard scope defers per-dashboard sharing — not invented here.
- **No `if cloud {…}`.** The same workbench the `workstation` runs in Tauri is the app the `hub` serves
  to a `browser` — delivery differs (Tauri `invoke` vs SSE/HTTP), the app and the host verb do not.
- **No `*.fake.ts`.** Tests drive a real in-process gateway seeded via the real write path (the
  retirement rule).
- **No model runtime.** The Playground's `ai.*` calls the **already-wired** AI seam; a workspace with no
  model configured renders the shipped clear **"AI not configured"** state (the host's `DisabledModel`
  error), never a fake completion. The model provider behind the gateway is the one external.

## Intent / approach

**Mirror the dashboard surface verb-for-verb; add no host work.** The dashboard surface is the proven
template: a `crates/host/src/dashboard/` service → a `routes/dashboard.rs` gateway mirror (re-check cap,
token-derived ws) → a `lib/dashboard/dashboard.api.ts` client (one call per export, 1:1 with the verbs)
→ a `features/dashboard/` React surface. The rules workbench is the **same four layers minus the host
layer** (it exists): the **gateway routes** (`rules.rs`, `chains.rs`, `datasources.rs`), the **API
clients** (`rules.api.ts`, `chains.api.ts`, `datasource.api.ts`), and the **React surface**
(`features/rules/`, `features/datasources/`). The UI goes through named verb clients (`invoke(...)`),
never a raw call — exactly like `dashboard.api.ts`.

```
  features/rules/RuleEditor (CodeMirror)  ──rules.run {body|rule_id, params}──►  RunResult
        │                                          │ by output.kind
        │  Save ──rules.save──► rule:{ws}:{id}      ├─ scalar → ScalarCard
        │  rail ──rules.list/get/delete──►          ├─ grid   → GridTable
        ▼                                           └─ findings→ FindingsList  (+ log + ms/ai budget)
  gateway routes/rules.rs  (re-check mcp:rules.<verb>:call, ws from token)
        ▼
  host rules.* service (SHIPPED) → cage + per-source caps::check inside every collect

  features/rules/ChainCanvas (@xyflow/react)  ──chains.run──► {run_id}
        │   nodes=steps, edges=needs                  │
        │   Save ──chains.save (validated up front)   │  poll
        ▼                                             ▼
  gateway routes/chains.rs                    chains.runs.get → colour nodes (pending/running/ok/err/skipped)

  features/datasources/DatasourcesAdmin  ──datasource.add/list/test/remove──►
        ▼                                              (DSN → secret.set host-side; never echoed back)
  gateway routes/datasources.rs → host federation.* / datasource.* service (SHIPPED)
        │  add/list/remove → datasource:{ws}:{name} store record (secret REF, never DSN)
        └  test → host net:* pre-connect + DSN-mediate → SUPERVISED federation sidecar → green/red
```

- **State vs motion held (rule 3).** The rule body, the saved chain DAG, a datasource record are
  **state** (`rules.get`/`chains.get`/`datasource.list` — store reads). A single rule **run** is bounded
  and synchronous (returns its result). A chain **run's progress** is motion — surfaced in Phase 2 as a
  `chains.runs.get` **snapshot poll** (the durable per-step records the run-store rebuilds), with the
  `chains.watch` **SSE** as the named follow-up (the dashboard "live fan-out deferred" pattern). Never a
  `setInterval` poll dressed up as "live" beyond the explicit settle-poll bound below.
- **The cage/deny IS the UI's honesty test.** A run that hits the wall returns a typed error, and the
  page must render it as itself: a **denied source** (the invoker lacks `store.query`/`federation.query`
  — `caller ∩ grant`) → "this rule reads a source you can't access"; an **AI-budget** abort → "AI budget
  exceeded for this run"; a **cage** error (`eval`/`import`/loop/oversize) → the parse/runtime message;
  a **cyclic DAG** at `chains.save` → an inline edge error. Never a blank, never a fake value (the
  dashboard "honest denied/empty, never fake" rule applied to rule results).
- **The DSN never round-trips to the page.** Adding a datasource supplies a DSN; the host's shipped
  `datasource.add` writes it via `lb_secrets::set` and stores only a **secret ref** on the
  `datasource:{ws}:{name}` record. `datasource.list` returns the ref, never the value; a query result
  never carries it. The page holds the DSN only in the Add form's submit, never reads it back — a
  redaction assertion proves it.

**Rejected alternatives:**

- *Give the federation extension a UI.* Rejected — it's a headless native sidecar (sockets + engine +
  pools behind a `net:*`+secret wall); admin UI belongs in the trusted shell, not the least-trusted
  process. The page drives the host verbs.
- *Monaco for the editor / a new graph lib for the canvas.* Rejected — `@uiw/react-codemirror` and
  `@xyflow/react` are already dependencies; adding Monaco/another graph lib is weight for nothing.
- *`chains.watch` SSE in Phase 2.* Rejected for v1 — the durable `chains.runs.get` snapshot poll is
  honest and simpler; the SSE is the named scaling follow-up (matches dashboard's deferred multiplexed
  stream). Building the SSE first over-builds the live transport before the canvas is proven.
- *A bespErase per-rule React tree / poll `rules.list` for "live".* Rejected — one generic editor +
  result renderer driven by the verb responses, like the dashboard is one generic grid host.
- *Echo the DSN back for "edit".* Rejected — the DSN is write-only to the secret store (rule: a secret
  is never returned to a page/log/record). Editing an endpoint re-supplies the DSN; the page never reads
  it.

## How it fits the core

- **Tenancy / isolation (rule 6):** every `rule:{ws}:{id}`, `chain:{ws}:{id}`, `datasource:{ws}:{name}`
  is workspace-namespaced; the host verbs are workspace-first. A ws-B user cannot list/get/run/save/
  delete a ws-A rule or chain, cannot resolve a ws-A datasource, and a ws-B rule's data reads hit only
  ws-B (the shipped series/federation wall). The two-session isolation test extends to the workbench.
  **Mandatory test** (at the UI boundary + the gateway).
- **Capabilities (rule 5/7):** the page is a **caller** of existing caps — no new caps. Nav + actions
  gate on the **shipped** `mcp:rules.{run,save,get,list,delete}:call`,
  `mcp:chains.{save,run,get,list,delete}:call` (+ the `chains.runs.get` read),
  `mcp:datasource.{add,remove,list,test}:call` / `mcp:federation.query:call`. The **UI gate is
  convenience** (hide a button a user can't use); the **gateway re-checks every cap server-side** (the
  real wall). Inside a rule run, every data verb still hits `caps::check` under `caller ∩ grant` — the
  page cannot widen it. The deny is opaque where it's a cap, explanatory where it's author feedback. **A
  deny-test per verb at the gateway** (mandatory, HOW-TO-CODE §3 step 4a).
- **Placement / symmetric nodes (rule 1):** one app, two deliveries — Tauri-local on the `workstation`,
  served over SSE/HTTP from the `hub` to a `browser`. The `rules.*`/`chains.*`/`datasource.*` routes are
  role-mounted by config (the gateway already mounts by role), never `if cloud`.
- **MCP surface — consumed, not added (§6.1):** this scope **consumes** the shipped verbs; it adds **no
  MCP tools**. The shapes it drives:
  - **CRUD:** `rules.save`/`rules.delete`, `chains.save`/`chains.delete`, `datasource.add`/
    `datasource.remove` — each already its own host verb + cap. The page calls them; it defines none.
  - **Get / list:** `rules.get`/`rules.list`, `chains.get`/`chains.list`, `datasource.list`,
    `chains.runs.get` (the per-step snapshot the canvas reads).
  - **Run (bounded, synchronous):** `rules.run` returns its result inline (the governors bound it);
    `chains.run` returns a `run_id` (a chain is a job) the canvas then polls.
  - **Live feed:** **deferred** to the `chains.watch` SSE follow-up; Phase 2 uses the `chains.runs.get`
    snapshot poll (bounded — see Risks). For a *single* rule run there is no feed (it returns its
    result). Stated as the deliberate Phase-1 transport, not a gap.
  - **Batch:** **N/A** — a user edits/runs one rule or one chain at a time; a chain *is* the batch-as-job
    (`chains.run`). No bulk workbench verb. Stated per §6.1, not a silent omission.
- **Data (SurrealDB):** **no new tables.** The page reads/writes the shipped `rule:{ws}:{id}`,
  `chain:{ws}:{id}` + `chain_run`/`chain_step`, and `datasource:{ws}:{name}` records via the host verbs.
  Layout/editor state is **not persisted client-side** — the rule body is the record; no `localStorage`
  durable state (rule 4). Transient editor buffer (unsaved text) is fine in component state.
- **Bus (Zenoh):** none new in Phase 1/2 (the snapshot poll reads records). The `chains.watch` follow-up
  would bridge the per-run status subject `ws/{ws}/chain/{run}/**` over SSE (the dashboard series-stream
  pattern) — named, not built here.
- **Sync / authority:** the rule/chain/datasource records ride the **existing** §6.8 `(table,id)` upsert
  sync path (no new mechanism). A rule authored on the hub appears on a workstation; a chain run is
  authoritative on its hosting node and resumes idempotently (shipped). The UI reads whatever the node
  is authoritative for.
- **Secrets:** the **only** secret material is the datasource DSN (Phase 3). It flows **page → host →
  `lb_secrets::set`** at `datasource.add` (already shipped) and is **never** returned to the page, a log,
  a record, or a query result. The page holds only the secret **ref**. A redaction assertion is
  mandatory.

## Example flow (Phase 1 — the Playground)

1. **Open.** Alice (member of `kfc`, holding `mcp:rules.*` + `mcp:store.query`/`series.read`) logs in via
   the browser. The shell reads her caps from the session token; the **Rules** nav slot shows (cap-gated
   on `mcp:rules.run:call`). Bob in `mcdonalds` sees none of `kfc`'s rules (the wall).
2. **Write.** Alice types a Rhai body in the CodeMirror editor:
   ```
   let hot = history("series", "cooler.temp", "24h").filter("value > 5.0");
   if hot.size() > 0 { alert(#{ level: "critical", series: "cooler.temp", msg: "hot" }); }
   ```
3. **Run.** She hits **Run** → `rules.run {body, params:{}}`. The result renders as a **findings** list
   (one `critical` alert, `alert`-marked), with the **log** lines and the **budget** (`ms`, `ai: 0
   calls`). The alert also raised a real inbox item (shipped host behaviour) — surfaced as a small "1
   alert raised" note.
4. **Deny (honest).** Alice removes her `store.query` grant and re-runs → the page shows **"this rule
   reads a source you can't access"** (the `caller ∩ grant` deny), not a blank or a fake grid. A body
   with `eval("1+1")` shows the **cage** error; a 100× `ai.complete` loop shows **"AI budget exceeded."**
5. **Save.** **Save** → `rules.save {id:"cooler-foodsafety", name, body, params}` persists
   `rule:kfc:cooler-foodsafety`; the left rail now lists it. Bob cannot see or run it.
6. **Reopen / delete.** Clicking the rail item loads it via `rules.get`; **Delete** tombstones it via
   `rules.delete` and the rail drops it (re-delete is a no-op).

## Example flow (Phase 2 — the chain canvas)

1. Alice opens a chain (or creates one). The **React Flow** canvas shows nodes for `pull → roll →
   summarize → notify` with `needs` edges. She adds an edge that would create a cycle → `chains.save`
   returns the validation error → the canvas shows an **inline "cycle" error**, no save.
2. She fixes it, **Save** persists the DAG (`chains.save`, validated up front). **Run** → `chains.run`
   returns `{run_id}`. The canvas polls `chains.runs.get`: `pull` goes **Running → ok** (green), `roll`
   **Running**, … On a step failure under Halt, the failed node is **red** and its subtree **greyed
   (skipped)**; the banner shows `partialFailure`. A late open of the run rebuilds the same colours from
   the snapshot (the records are the source of truth).

## Example flow (Phase 3 — datasources admin)

1. The `kfc` admin opens **Datasources** (cap-gated on `mcp:datasource.list:call`). `datasource.list`
   shows the registered sources — kind + endpoint + a **redacted** secret ref, never a DSN.
2. **Add** → a form for `{name:"timescale", kind:"postgres", endpoint:"tsdb.acme:5432", dsn:…}`. The
   form shows the implied grants (`net:tls:tsdb.acme:5432:connect`, `secret:federation/timescale:get`).
   Submit → `datasource.add` writes the record + the DSN to the secret store (host-side); the page never
   reads the DSN back.
3. **Test** → `datasource.test {source:"timescale"}` → the host runs `net:*` pre-connect, mediates the
   DSN, probes via the supervised sidecar → a **green** badge (or **red** with the error). **Remove** →
   `datasource.remove` drops the record.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` (real infra, seeded via the real write path —
**no mock data, no `*.fake.ts`**; the frontend tests drive a **real in-process gateway** seeded with
real rows, per STATUS item 00). The seed reuses the shipped **`seed_iot_demo`** (real series) plus a
seeded saved rule/chain through the real `rules.save`/`chains.save` path.

- **Capability deny — per verb (gateway + UI).** A principal without `mcp:rules.{run,save,get,list,
  delete}` / `mcp:chains.{save,run,get,list,delete}` / `mcp:datasource.{add,remove,list,test}` is refused
  that route server-side (nothing read/written); the UI hides the action and renders an opaque deny if
  forced. **One deny-test per verb** (mirrors the dashboard per-verb deny set).
- **Workspace isolation.** Two real sessions: a ws-B principal cannot list/get/run/save/delete a ws-A
  rule or chain, cannot resolve a ws-A datasource; the workbench rosters are workspace-partitioned.
  Across **gateway + store**.
- **The cage/deny renders honestly (the UI honesty test — this slice's headline).** A rule that reads an
  ungranted source renders the **denied-source** state (not a fake grid); an `eval`/loop/oversize body
  renders the **cage** error; a `for`-loop of `ai.complete` renders **AI-budget-exceeded**; a workspace
  with no model renders **"AI not configured."** Each asserted at the UI boundary against the real host
  responses — no fabricated success.
- **Offline / sync.** The `rule`/`chain`/`datasource` records ride the §6.8 upsert path idempotently
  (reused, not rebuilt) — assert a saved rule/chain replays once; don't build a new mechanism.

Plus this slice's specific cases:

- **Run round-trip (the three output kinds).** A scalar rule → ScalarCard; a returned-grid rule →
  GridTable with the right columns/rows; an `emit`/`alert` rule → FindingsList (level colours, alert
  mark) + the log + the `ms`/`ai` budget. Against the real gateway + seeded series.
- **CRUD round-trip.** `rules.save` (create) → rail shows it → `rules.get` loads it → edit + save
  (update, same id) → reflected → `rules.delete` → rail drops it → re-delete is a no-op. (And the chain
  equivalent in Phase 2.)
- **Chain settle-colouring (Phase 2).** A seeded diamond chain run colours `ok` nodes green, a Halt
  failure red + its subtree greyed; a late `chains.runs.get` rebuilds the same state from records.
- **Datasource probe + redaction (Phase 3).** `datasource.add` → `datasource.test` returns green against
  a real spawned DB (reuse the federation test harness); the **DSN never appears** in `datasource.list`
  output or any response (redaction assertion).
- **Vitest (frontend), real in-process gateway, seeded real rows:**
  - `RuleEditor` runs a seeded rule and renders each output kind; the deny/cage/budget/AI-not-configured
    states render from real host errors.
  - the rail lists `rules.list`, opens `rules.get`, deletes via `rules.delete`.
  - (Phase 2) `ChainCanvas` renders a seeded DAG, rejects a cyclic edge inline, colours a run from
    `chains.runs.get`.
  - (Phase 3) `DatasourcesAdmin` lists/adds/tests, shows the implied grants, never displays a DSN.
  - the deny + isolation cases at the UI boundary (a ws-B view shows no ws-A rules/chains/datasources).

## Risks & hard problems

- **Settle-poll cadence (Phase 2).** Polling `chains.runs.get` colours the canvas; an over-eager poll
  hammers the node. Phase-2 bound: poll on a **fixed interval while the run is non-terminal, stop on a
  terminal status**, with a sane interval (config) and a max duration; a late open does **one** snapshot
  read. The `chains.watch` SSE replaces the poll later (named follow-up). Watch the request count in the
  settle test.
- **Honest failure rendering is the load-bearing UX.** The whole point is that a denied source, a budget
  abort, a cage error, a cyclic DAG each render as **themselves** — the temptation to swallow an error
  into a generic "run failed" toast is the anti-pattern. The result component must switch on the typed
  error class (the host's `RulesError`/`ChainsError` → `ToolError` mapping: `BadInput` = author
  feedback shown verbatim; `Denied` = a generic "not permitted"). This is the dashboard "render the deny,
  not a blank" rule, and it's the slice's headline test.
- **Grid result size.** A rule returning a large grid could render a huge table. Bound the rendered rows
  (a `head`/paging in the GridTable, the host already row-caps `store.query`); show "showing N of M."
- **Editor ↔ saved divergence.** The editor buffer can drift from the saved record (unsaved edits). Show
  a dirty indicator; a run uses the **buffer** (ad-hoc `body`), a save persists it — never silently run
  the saved version when the buffer differs.
- **React Flow ↔ record mapping (Phase 2).** The canvas node/edge model must map **1:1** to the
  `chain.steps[].needs` record (like the dashboard cells ↔ `react-grid-layout` items map 1:1), so a save
  is a faithful serialization and a load is a faithful render — no canvas-only state that the record
  can't hold.
- **The DSN must never leak to the page (Phase 3).** The single secret-handling risk: the Add form is
  the only place a DSN exists client-side, and only on submit. `datasource.list`/`test` responses must be
  asserted DSN-free. Treat any DSN in a response as a bug, not a convenience.

## Open questions

Decisions are **made** below so each phase codes with no open question (HOW-TO-CODE §3 step 4a); the
residual opens are explicitly future phases or named follow-ups, not gaps.

**Resolved (decisions taken):**

- **Editor:** `@uiw/react-codemirror` with `lang-javascript` highlighting (Rhai is JS-like; the dep is
  already present). **Not** Monaco. Decided.
- **Canvas:** `@xyflow/react` (React Flow v12, already a dependency) for the chain DAG. Decided.
- **Run-progress transport (Phase 2):** the `chains.runs.get` **snapshot poll** (bounded), **not** a new
  SSE in v1; `chains.watch` SSE is the named follow-up. Decided.
- **Output rendering:** three components keyed on `RuleOutput.kind` — `ScalarCard` / `GridTable` /
  `FindingsList` — plus a `LogPanel` and a `BudgetBadge`. One per file (FILE-LAYOUT). Decided.
- **DSN entry:** supplied in the Add form, written via the host's shipped `datasource.add` →
  `lb_secrets::set`; **never** read back (the page holds the secret ref only). Decided (and already the
  host behaviour).
- **No new caps, no host changes:** the page is a caller of shipped verbs; the work is gateway routes +
  clients + React. Decided.
- **The federation extension stays headless** — the datasources page is first-party shell code. Decided
  (a non-goal).

**Named follow-ups (out of the build-ready phases, not silent gaps):**

- **`chains.watch` live SSE** — the host + gateway live feed replacing the Phase-2 settle poll (the
  `run_stream.rs`/`series_stream.rs` pattern). A small additive host slice.
- **Per-rule / per-chain sharing (Phase 1.5)** — reuse the S4 asset model (private → team → workspace),
  exactly as the dashboard defers per-dashboard sharing. Workspace + cap is the build-phase boundary.
- **A Playground "explain this deny" affordance** — surfacing *which* source/cap a run lacked (the host
  already classifies it; richer UI is additive).
- **Datasource edit (vs remove+add)** — re-supplying a DSN to change an endpoint; v1 is add/remove.

**Phase ordering:** all three phases **shipped 2026-06-29** in one session (Playground, chain canvas,
datasources admin) — built as three parallel vertical slices over the same gateway-route + client +
React pattern. The named follow-ups above (`chains.watch` SSE, per-rule/chain sharing, the
"explain this deny" affordance, datasource edit) remain follow-ups. One thing the scope didn't
anticipate: the shipped host `rules.list`/`chains.list` verbs dropped every row by not unwrapping the
`{data}` store envelope — fixed this session with a regression test
(`debugging/host/rules-chains-list-drops-every-row-envelope.md`), since the rail/CRUD round-trip the
scope specifies is impossible otherwise.

## Related

- `scope/frontend/dashboard-scope.md` — the surface this mirrors verb-for-verb (trusted-shell core,
  gateway mirror, `*.api.ts` 1:1, real-gateway Vitest, cap-gated nav, "render the deny not a blank").
- `scope/rules/rules-engine-scope.md` — the shipped `rules.*` engine + the **Playground** this builds;
  the cage/deny/budget/fence states the UI renders honestly.
- `scope/rules/rule-chains-scope.md` — the shipped `chains.*` DAG + the **canvas** this builds; the
  `chains.runs.get` snapshot the settle-colouring reads; the `chains.watch` SSE follow-up.
- `scope/datasources/datasources-scope.md` — the shipped `datasource.*`/`federation.*` verbs the
  **datasources admin page** drives; why the federation extension is headless (the `net:*`+secret wall).
- `scope/extensions/ui-federation-scope.md` + `scope/frontend/dashboard-widgets-scope.md` — the
  *extension*-contributed UI path (a widget/page a remote ships); explicitly **not** how the datasources
  page works (it is first-party shell, the contrast that keeps the trust model right).
- `scope/files/files-scope.md` — the S4 asset sharing the Phase-1.5 per-rule sharing reuses.
- `scope/testing/testing-scope.md` — the real-gateway, real-seed, no-`fake.ts` discipline.
- README **§6.5** (MCP — the contract), **§6.12/§6.13** (one-app-two-deliveries + the gateway SSE/HTTP
  path), **§3** (the non-negotiables — state vs motion, workspace wall, capability-first, symmetric
  nodes).
- Sessions of the shipped backend: `sessions/rules/rules-session.md`,
  `sessions/datasources/{datasources,federation}-session.md`.
```
