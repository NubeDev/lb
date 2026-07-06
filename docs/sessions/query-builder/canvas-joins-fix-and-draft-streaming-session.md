# Session — canvas-builder join fixes, Rules joins, query-draft streaming, run history

Date: 2026-07-06 · Branch: `insights-v1` · Scope docs:
[`visual-canvas-builder-scope.md`](../../scope/frontend/query-builder/visual-canvas-builder-scope.md) (fixes),
[`query-draft-streaming-scope.md`](../../scope/frontend/query-builder/query-draft-streaming-scope.md) (new).

## 1. Canvas builder (slice 1) — three live-reproduced bugs, fixed

Reproduced against the real dev node (timescale datasource) before touching code:

| Bug (as reproduced) | Root cause | Fix |
|---|---|---|
| Second canvas table rendered EMPTY (`site_tag` — header only) | `useFederationSchema` lazy-described only ONE table (the FROM table) | Hook now takes `string \| string[]`; every referenced table (FROM + each join) gets its own keyed describe into a per-table cache (+ epoch guard against cross-source staleness). Hosts pass `[table, ...joins]`. |
| SQL preview showed `… CROSS JOIN "site_tag"` for an unwired table | `renderJoin` treated a PENDING join (`inner`, `on:[]` — what "Add table" produces) as CROSS | `isPendingJoin` (in `query.ts`, shared by emitter + canvas): a pending join and every column/filter/sort/group-by on its table are excluded from the emitted SQL. |
| No visible way to create a join (handles `opacity: 0`, measured live) | Hover-only handle reveal — undiscoverable | Handles always faintly visible (full on row hover); "drag between column dots" hint when ≥2 tables and no edge; pending nodes get dashed-amber "not joined" badge; empty columns show "loading columns…". |

Also wired per the scope's required behaviour: edge delete → `disconnectJoin` (table returns to
pending), node delete (X or keyboard) → `removeTable` (drops join + columns + filters + **sorts +
group-bys**), edges re-derive from the model so the type-cycle click always resolves its join.

## 2. Rules (row-list) builder — joins added

The Rules body had NO join affordance at all ("I can't add joins"). New `JoinRows.tsx`
(`type · table · on left = right` per join, `+ add join`), gated on `dialect === "standard"`.
Half-picked ON keys are VIEW state (a draft) — the model carries `on: []` (pending, out of the SQL)
until both columns are chosen. Columns/Group-by/Order-by selects are join-aware (`table.column`,
the same convention `filterQueryBuilder.ts` already used).

## 3. Query-draft streaming (agent → open builder over SSE) — new slice, shipped

Per the scope: the agent publishes full `SqlSourceState` frames on
`querybuilder/<source>/draft` via the SHIPPED `bus.publish`; the workbench follows over the
SHIPPED `/bus/stream` SSE (`useQueryDraftFollow` → `parseDraftFrame` → replace editor state) and
shows a "live draft" pill. Canvas/Rules/Code all follow because they are projections of the one
model. Verified live: three frames (table → join → columns) drove the open canvas, then Run
returned 80 real rows.

## 4. Run history (last 10 unique, restore)

`runHistory.ts` (localStorage fold keyed by `(ws, source)`, dedupe-by-SQL, cap 10) +
`RunHistoryMenu` in the run bar; restore drops the SQL into Code mode. Verified live incl.
dedupe-moves-to-front and reload persistence.

## 5. Host SQL gate — leading comments falsely rejected

Reported as "JOIN isn't allowed" (`rejected sql: only SELECT/WITH allowed`). JOINs were fine
(live-verified: the canvas JOIN ran, 80 rows); the trigger is a statement not literally starting
with `SELECT`/`WITH` — e.g. an agent's `-- header` comment. `validate_select_host` now strips
leading `--`/`/* */` comments before the leader check (a comment can still not hide a write —
the leader after stripping is still gated; the sidecar parser stays authoritative).
Debug entry: [`docs/debugging/datasources/host-select-gate-rejects-leading-comment.md`](../../debugging/datasources/host-select-gate-rejects-leading-comment.md).

## 6. Small UX fix

`VisualEditor` picked its Canvas/Rules default at mount, when the async schema was still empty —
so Canvas never became the default. It now upgrades to Canvas when availability arrives, unless
the user has explicitly picked a mode.

## Tests (all green; run this session)

- Unit: `canvasModel.test.ts` (14 — incl. pending-node, disconnect/remove-table cleanup),
  `toStandardSql.test.ts` (26 — incl. 3 pending-join goldens), `queryDraft.test.ts` (5),
  `runHistory.test.ts` (5), plus the untouched sql/rules suites.
- Gateway (real node): `QueryDraftFollow.gateway.test.tsx` — headline follow, malformed-frame
  drop, **capability-deny** (403 pre-body, silent degrade), **workspace-isolation** (ws-B publish
  never reaches ws-A). Uses `ui/src/test/eventsource-shim.ts` (fetch-backed EventSource polyfill
  against the REAL gateway — jsdom has none; a browser-API shim, not a fake backend).
- Rust: `cargo test -p lb-host --lib federation::validate` (6, incl. the new comment-gate test).
- Known pre-existing failures not chased: `SystemView.gateway`, `sqlSource.gateway`,
  `agent_routed_test`, `FlowsCanvas.gateway`/`transformDebug` tsc errors, `NavRail.tsx` tsc
  errors (user's in-flight branding work).

## Open / handed back

- **Agent "no response" after tool calls** (dock: tools ran ✓ but no final answer). Reproduced via
  `POST /agent/invoke`: the run returns only the FIRST assistant sentence — the model's post-tool
  turns come back with empty content, so `last_content` never advances past the intro. This is the
  agent loop / provider think-stripping seam (`host/src/agent/run.rs` already keeps
  last-non-empty; the missing piece is a final "answer now" nudge or provider-side fix) — separate
  scope, NOT part of this session's changes.
- The Rust validator fix needs a node rebuild (`make kill && make dev`) to be live.
