# Channels scope — in-channel SQL query with auto-plotted charts

Status: scope (the ask). Promotes to `public/channels/` once shipped.
Topic: `channels`. Builds on `channels-scope.md` (the registry/history/stream surface) and the shipped
`federation.query` verb (`scope/datasources/datasources-scope.md`).

A channel member runs a SQL query against a registered datasource by posting it into a channel. A
host-side worker executes the query through the existing `federation.query` capability, persists the
result as a structured channel `Item`, and publishes it as live motion. The UI renders the result as a
table and, when the shape allows, **auto-plots a chart** — turning a channel into a shared, durable,
auditable query notebook where the query, who ran it, and the result all live in history together.

## Goals

- Post a SQL query into a channel and get the result back **in the same channel**, durably.
- The query, the runner's identity, and the result are one ordered history a whole channel can scroll.
- Results auto-plot a chart when the column shape is plottable; otherwise fall back to the table.
- Reuse the channel transport (history + bus + SSE) and the `federation.query` gate — no new transport,
  no new datasource path.
- Access stays capability-first and workspace-scoped: running a query needs *both* channel `pub` and a
  datasource grant.

## Non-goals

- No new query engine, dialect, or SQL parsing — `federation.query` is the only execution path.
- No chart *editor* / dashboard builder. Auto-plot picks a sensible default; rich charting is the
  separate dashboards scope (`scope/.../dashboards`), which this can later feed.
- No write SQL. SELECT-only, enforced host-side exactly as `federation.query` already does.
- No new channel kinds machinery beyond the single `Item` payload tag this needs (see Open questions).
- No cross-channel result sharing or pinning in this slice.

## Intent / approach

A channel `Item` payload is opaque, so we use it as a typed envelope. Three message shapes share the
channel, distinguished by a `kind` field on the payload:

- `kind: "query"` — `{ source, sql }`, posted by a member who wants to run a query.
- `kind: "query_result"` — `{ source, sql, columns, rows, chart }`, posted by the worker.
- `kind: "query_error"` — `{ source, sql, error }`, posted by the worker when the query fails.
- (absent / `kind: "chat"`) — an ordinary message, rendered as text. Untagged stays chat, so this is
  purely additive and existing channels are unaffected.

The flow is **request/response over the channel**: a `query` item is the request; a `query_result` (or
`query_error`) item posted in reply is the response. Because both are durable `Item`s, the full
exchange is permanently in `history` and streams live over the existing SSE route. No request ever
blocks a connection — the requester sees the answer arrive as motion, same as any other message.

**Where the work runs:** a host-side channel **query worker** subscribes to channel motion (or is
invoked inline by `post` when it sees a `query` item — see Open questions), gates the call through
`federation.query`, and posts the result back. We keep the worker host-side rather than in the browser
so the datasource capability check and SELECT-only validation are never client-trusted, and so an AI
agent or another extension can issue queries the same way (rule 7).

**Chart selection** is deterministic and host-computed into the `query_result.chart` field so every
subscriber renders the same chart without re-deriving it: pick the first non-numeric/temporal column as
the category/x axis and the numeric columns as series (line for a temporal x, bar for a categorical x);
if there is a single numeric column and many rows with no obvious category, fall back to a histogram;
if nothing is plottable, `chart: null` and the UI shows the table only. The rule lives in one file and
is unit-tested against fixed row-sets.

**Alternative considered — run the query client-side and post only the result.** Rejected: it moves the
capability/SELECT-only gate into the browser (a client could fabricate a `query_result` for a source it
can't read), and it bypasses the durable request item, losing the "who asked what" audit. Posting the
request and letting a host worker answer keeps both gates host-side and the audit complete.

## How it fits the core

- **Tenancy / isolation:** unchanged from channels — store reads use the workspace namespace and bus
  keys are workspace-prefixed by `lb_bus`. The datasource is resolved host-side within the workspace; a
  ws-B member can neither post into a ws-A channel nor name a ws-A source.
- **Capabilities:** running a query requires **two** grants, checked in order — channel `bus:chan/{cid}:pub`
  to post the `query` item, then `federation.query`'s existing datasource grant when the worker executes.
  A member with channel `pub` but no datasource grant gets a `query_error` item (opaque: "not permitted",
  not "source X exists"). A member with `sub` but not `pub` can *see* results others ran but cannot run
  their own. The deny path is the standard opaque host deny — see Testing plan.
- **Placement:** either. The worker is symmetric host code (no `if cloud`); it runs wherever the channel
  and the federation sidecar run. Edge vs cloud is the usual config/role difference.
- **MCP surface** (API shape, §6.1):
  - **Create (the only write):** no *new* tool — a query is `channel.post` with a `kind:"query"` payload.
    Reuses the existing `post` verb + `bus:chan/{cid}:pub` gate; the worker reuses `federation.query`.
    We deliberately add no `query_create` tool: the channel `post` verb already is the create.
  - **Get / list:** none new — `channel.history` already returns all items including results; the UI
    filters by `kind`. State the workspace-scoped read uses the existing `bus:chan/{cid}:sub` gate.
  - **Live feed:** none new — the existing `subscribe_channel` / SSE `event: message` carries
    `query_result` items. The UI distinguishes by payload `kind`; no new SSE event type.
  - **Batch:** N/A for this slice. One query per item. (A future "run these 5 queries" could become a
    job, but a single SELECT is bounded and fast — it stays the synchronous `federation.query` call the
    worker already makes.)
- **Data (SurrealDB):** the `query_result` `Item` is persisted to `lb_inbox` exactly like any channel
  message — same table, same workspace namespace. No new table. Result rows live *inside* the item
  payload (bounded by a row cap — see Risks), they are **not** a new persisted dataset; SurrealDB stays
  the authority for the *channel*, the external source stays authority for the *data*.
- **Bus (Zenoh):** reuses `chan/{cid}/msg/**`. Result motion is **replay-class**: it is already
  persisted to the inbox before publish (the `post` path), so a missed live frame is recovered by
  `history`. No new subject.
- **Sync / authority:** node-local channel store as today; the external datasource is queried live and
  is its own authority. Offline: a `query` item posted offline is durable and the worker answers when
  the node and sidecar are reachable — no special-casing.
- **Secrets:** none new client-side — the DSN never leaves the secret store; the worker resolves the
  source by name through `federation.query`, which already mediates the secret (redaction rule §6.7).
- **State vs motion:** result is state (inbox `Item`) first, motion (bus publish) second — same order
  as every channel post.
- **Stateless worker:** the query worker holds no durable state; everything is in the inbox item or on
  the bus. Hot-reload safe.

## Example flow

1. Alice (holds `bus:chan/data:pub` and `federation.query` for source `warehouse`) types
   `/query warehouse | SELECT day, signups FROM daily ORDER BY day` in channel `data`.
2. The UI posts an `Item` with payload `{ kind: "query", source: "warehouse", sql: "SELECT day, signups …" }`
   via the existing `POST /channels/data/messages`. The `post` gate checks `bus:chan/data:pub` — pass.
   The item is persisted and published; everyone in `data` sees the query appear (rendered as a query
   chip, not raw text).
3. The host query worker sees the `query` item, calls `federation.query(source="warehouse", sql=…)`.
   The datasource grant is checked and SELECT-only re-validated host-side — pass.
4. The worker builds the result payload: `columns: ["day","signups"]`, the rows, and
   `chart: { type: "line", x: "day", series: ["signups"] }` (temporal x, one numeric series). It posts a
   second `Item`, `kind: "query_result"`, into `data` (its own identity holds `pub`).
5. The result item persists and publishes. Over SSE, every `data` subscriber receives `event: message`
   with the `query_result`. The UI renders a results table **and** an auto-plotted line chart inline.
6. Bob, subscribed to `data` with `sub` only, sees Alice's query and the chart in history when he scrolls
   back later — the whole exchange is durable. Bob cannot run his own query (no `pub`); if he tries, the
   `post` gate denies opaquely.
7. Carol runs `SELECT * FROM huge_table` (no useful chart shape); the worker sets `chart: null` and the
   UI shows the table only. If Carol lacks the `warehouse` grant, the worker posts a `query_error` item
   "query not permitted" — opaque, no source-existence leak.

## Testing plan

Per `scope/testing/testing-scope.md`; no mocks — real store (`mem://`), real bus, real gateway, a real
seeded datasource (sqlite `mem://` source is enough for the federation path).

- **Capability deny (mandatory):**
  - member with channel `sub` but not `pub` posting a `query` item → host deny (opaque).
  - member with channel `pub` but **no** datasource grant → worker posts a `query_error` "not permitted";
    assert it does **not** reveal whether the source exists.
  - non-SELECT SQL in a `query` item → rejected host-side (reuse the federation SELECT-only test path).
- **Workspace isolation (mandatory):** ws-B identity cannot post a `query` into a ws-A channel, cannot
  read ws-A `query_result` history, and naming a ws-A source name from ws-B resolves to nothing (no
  cross-workspace source access). Mirror `gateway_test.rs` ws-A/ws-B structure.
- **Chart selection (unit):** the picker against fixed row-sets — temporal x → line; categorical x +
  numeric → bar; single numeric column many rows → histogram; all-text / single-row → `chart: null`.
  One file, table-driven.
- **Integration (real gateway):** post a `query` item via `POST /channels/{cid}/messages`, assert a
  `query_result` item appears in `history` with the expected columns/rows/chart; assert it arrives live
  over the SSE stream as `event: message`. Extend `gateway_routes_test.rs`.
- **UI (real gateway, `*.gateway.test.tsx`, no fakes):** posting a query in `ChannelView` renders the
  query chip, then the result table + chart when the result item streams in; a `chart: null` result
  renders table-only; a `query_error` renders an inline error. Seed via the real gateway, per rule 9.

## Risks & hard problems

- **Result size.** Rows live inside a channel `Item` persisted to the inbox. A `SELECT *` on a big table
  would bloat history and the bus frame. **Mitigation:** the worker enforces a hard row/byte cap (e.g.
  ≤500 rows / ≤256 KB), truncates with a `truncated: true` flag, and the UI says "showing first N".
  Decide the cap before building (Open questions).
- **Worker identity & re-entrancy.** The worker posts `query_result` items, which are themselves channel
  posts — the worker must **not** treat its own result items as new queries (only `kind:"query"`
  triggers work). Guard explicitly and test it, or an infinite loop is one bug away.
- **Duplicate execution.** If the worker is driven by bus motion and the bus redelivers, a query could
  run twice. Tie execution to the durable `query` item id (idempotency key) so a redelivery is a no-op.
- **Chart over-promising.** Auto-plot must fail safe to the table, never render a misleading chart. The
  picker is conservative and `chart: null` is always acceptable.
- **Long-running queries.** A single SELECT is assumed bounded; a pathological query could hang the
  worker. The federation sidecar's own timeout bounds this — confirm a timeout exists and surface a
  `query_error` on it rather than a stuck channel.

## Open questions

- **Worker trigger:** inline inside `channel.post` (when it sees a `kind:"query"` item) vs. a separate
  bus-subscribed worker. Inline is simpler and idempotent by construction (one item → one execution in
  the post path); the subscriber is more decoupled but needs the dedup key. **Lean inline** for this
  slice — resolve in the session.
- **Row/byte cap value** for the result payload (proposed ≤500 rows / ≤256 KB) — pin the number.
- **`kind` field placement:** a top-level `Item` field vs. a reserved key inside the existing payload.
  Prefer reusing the existing payload with a `kind` key so no `Item` schema change is needed — confirm
  the `Item` shape allows it without a migration.
- **Chart payload schema:** the minimal `{ type, x, series, ... }` shape — finalize the fields so the UI
  renderer and the host picker agree (shared TS type in `ui/src/lib/channel/`).
- **Slash-command parsing** (`/query source | sql`) lives in the UI only; confirm we don't want a
  host-side parser (we don't — the UI builds the structured payload; the host never parses chat text).

## Related

- `scope/channels/channels-scope.md` — the channel registry/history/stream surface this builds on.
- `scope/datasources/datasources-scope.md` — the `federation.query` verb and SELECT-only/secret rules.
- `ui/src/features/datasources/useDatasourceQuery.ts` — the existing real `federation.query` client seam.
- `README.md` §6 (channels/bus), §6.7 (secret redaction), §6.10 (jobs — for the future batch case),
  §3 (the non-negotiables this honors).
- `docs/public/channels/channels.md` — promotion target on ship.
- Future: a dashboards scope can consume `query_result` items as panels.
