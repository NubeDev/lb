# Datasources scope — series raw-row paging (slice B, the fast path)

Status: **shipped** (2026-07-14, issue #56, `series-plane-readiness`) — **slice B** of
[`page-chaining-scope.md`](page-chaining-scope.md). See
[`../../sessions/ingest/series-plane-readiness-session.md`](../../sessions/ingest/series-plane-readiness-session.md).
Implementation notes: slice A's shared cursor crate is still unbuilt, so the opaque cursor codec ships
inside `lb_ingest::cursor` (versioned `v1`, `(seq, producer)` composite key — liftable into slice A
later). Default and max `limit` are both 10 000; direction is bidirectional (`fwd` default, `back`).
The 5M-sample perf gate / `.explain()` assertion is not in CI (indexes are defined + named:
`series_seq_idx`, `series_ts_idx`).

`series.read` today returns an **unbounded `Vec<Sample>`** for a `seq` range
([`ingest/read.rs`](../../../rust/crates/host/src/ingest/read.rs)): a big series either OOMs the call or
stalls the dashboard, and there is no way to ask for "the next page." This slice grows `series.read` into a
**keyset-paged** read — `{limit, cursor, direction, mode:"rows"}` in, `{rows, next_cursor, prev_cursor}`
out — that seeks the series plane's existing `(series, seq)` index in SurrealDB. This is **the reason the
whole feature exists**: dashboard-speed page loads over millions of committed samples, with flat latency at
any depth, composed with the live `series.watch` tail so backfill and live edge meet without a double-render
seam. This slice is native raw rows only (`mode:"rows"`); buckets/decimation is slice C.

> Read with: [`page-cursor-scope.md`](page-cursor-scope.md) (slice A — the cursor codec + keyset predicate
> this **consumes**, does not redefine), the parent's shared contract, `../ingest/ingest-scope.md` (the
> `series` plane + `series.read`/`latest`/`watch`). README `§3` (rules 2/3/5/6), `§6.1` (API shape).

## Goals

- Grow `series.read` (`series_read_range` in [`ingest/read.rs`](../../../rust/crates/host/src/ingest/read.rs))
  to accept `{limit, cursor, direction, mode:"rows"}` and return `{rows, next_cursor, prev_cursor}` — the
  unbounded `Vec<Sample>` becomes a bounded page. Additive: no new verb, no new capability.
- **Keyset-page** the read over the series plane's `(series, seq)` order key in SurrealDB — an index seek,
  **O(page)** and flat-latency at any depth, not an `OFFSET` scan that degrades every page.
- State the **index guarantee** the keyset seeks on, so the fast path is a promise a test enforces, not an
  accident of query planning — with **no new table**.
- **Compose cleanly with `series.watch`**: name how a caller backfills by paging *backward through committed
  state* and tails the *live forward edge* via the Zenoh watch (rule 3), meeting at a seam that renders each
  sample exactly once.
- Consume slice A's cursor + keyset primitives unchanged; every page re-authorizes workspace-first via the
  existing `mcp:series.read:call` cap.

## Non-goals

- **The cursor/keyset internals** — the opaque codec, the tiebreaker discipline, cursor versioning. Slice A
  ([`page-cursor-scope.md`](page-cursor-scope.md)) owns them; this slice *calls* them.
- **`mode:"buckets"` / decimation** — server-side time-bucketing for charts is slice C
  ([`series-decimation-scope.md`](series-decimation-scope.md)). This slice only serves raw rows.
- **External / federated sources** — pushdown paging and mirror-routing is slice D
  ([`federation-paging-scope.md`](federation-paging-scope.md)). This slice pages the **native** series plane.
- **The frontend** — infinite scroll / "load more" and the live-tail compose in the UI is slice E
  ([`page-chaining-ui-scope.md`](page-chaining-ui-scope.md)); this slice provides the host verb it calls.
- **Total counts / "page 42 of 1000"** and **unbounded export as a page loop** (that is a mirror/export job,
  §6.10) — both are whole-feature non-goals from the parent.

## Intent / approach

Turn the range read into a **keyset page** over the natural order key. `series_read_range`'s
`ORDER BY seq LIMIT n` gains a seek predicate: `direction:"back"` (the dashboard default — newest first)
resolves to `WHERE key < cursor ORDER BY key DESC LIMIT limit`; `direction:"fwd"` mirrors it. The `key` is
the series plane's **unique composite sort key** `(series, seq)` — `seq` is already the per-series monotone
commit counter, so `(series, seq)` is unique and the keyset never skips or repeats on a tie (slice A's
tiebreaker discipline; where a caller pages by wall-clock we seek `(ts, seq)`, `seq` breaking the `ts` tie).
The predicate and the opaque cursor that carries the last row's `key` are **slice A's primitives, imported,
not reimplemented here** — this slice's job is to wire them into `series.read` and guarantee the index.

**The index guarantee.** Keyset is only O(page) if the seek key is indexed; on an unindexed field
SurrealDB falls back to a full-series scan and the "fast path" is a lie that a small test never catches.
So this slice pins a **defined index on `(series, seq)`** on the series-plane table (and `(series, ts, seq)`
for the time-cursor variant) — the same natural order key `series.read` already returns in, so **no new
table and no new order**, just a named, tested index. A performance regression test (below) is what keeps it
honest: page-1 and page-500 latency stay in one band, and a `.explain()`/plan assertion proves the seek, not
a scan.

**Compose with `series.watch`, don't conflate (rule 3).** State lives in SurrealDB; motion lives on Zenoh.
A live chart does both: it **subscribes forward** to `series.watch` for new samples (motion) and **pages
backward** through `series.read` for history (state). The seam is defined by the newest committed `seq` at
subscribe time: the caller opens the watch, records the first live `seq` it sees, then backfills pages
*strictly older than that boundary*. Because keyset is **append-stable** — a head-append changes only the
newest page, never shifts an older one — concurrent ingest during backfill can't duplicate or skip a row
across the seam. Each sample is rendered exactly once: history from paging, everything at/after the boundary
from the watch.

**Rejected — `LIMIT/OFFSET` paging** (parent-level, restated for this read): O(offset) scan-and-discard
degrades on exactly the big series this exists for, and a head-append shifts every page. Keyset is
append-stable and O(page). **Rejected — a distinct `series.page` verb**: paging is additive shape on the
existing read, so the gate, the workspace resolution, and every guest already calling `series.read` come
along for free; a second verb would fork the cap and the tiebreaker discipline.

## How it fits the core

- **Tenancy / isolation:** the workspace and series come from the **token/request on every page**, never
  decoded from the cursor. The store read is namespace-scoped, so a ws-B reader physically cannot see ws-A's
  series (the existing hard wall in `series_read_range`). A ws-A cursor replayed under a ws-B token seeks in
  ws-B's namespace and resolves nothing — the cursor is a bookmark, not a key to another tenant.
- **Capabilities:** **no new cap.** The gate stays `mcp:series.read:call`, re-checked at the top of
  **every** page via the existing `authorize_ingest`. A grant revoked mid-chain denies the very next page —
  the cursor carries no authority and cannot bypass the check. The deny path is unchanged: a denied caller
  learns nothing.
- **Placement:** `either`, no `if cloud`. The same keyset code pages a local edge series and a mirrored
  cloud series identically; which series exist is config/grants, not a code branch.
- **MCP surface (§6.1):** additive `{limit, cursor, direction, mode:"rows"}` fields on the existing
  **get/list** read verb `series.read`; `next_cursor`/`prev_cursor` added to its result. No CRUD (reads are
  read-only), no new live-feed (the tail is the existing `series.watch`), no batch (a whole-range pull is a
  mirror/export **job**, §6.10, not a client page loop). `limit` is bounded with a default and a max.
- **Data (SurrealDB):** the committed series-plane table only — **no new table, no stored aggregates.** The
  change is a **named index** on the paging key `(series, seq)` (and `(series, ts, seq)`) plus a keyset
  predicate. State only; the live edge stays on the bus. Rule 2 intact.
- **Bus (Zenoh):** none of this slice's own — but it defines the **compose contract** with `series.watch`
  (motion): backfill is state (paging), live is motion (watch), joined at the subscribe-time `seq` boundary.
- **Sync / authority:** node-local read of committed state; no cross-node authority question. Paging walks
  already-committed samples, so it is deterministic under concurrent head-appends (append-stable keyset).
- **SDK/WIT impact:** **additive fields on an existing MCP read verb** — no new verb, no new host-callback,
  no ABI break. A guest already calling `series.read` gains paging by sending `limit`/`cursor`; one that
  doesn't send them keeps today's behavior (with the unbounded return now bounded to the default `limit`).

## Example flow

A dashboard opens a live chart over `series="floor2/temp"` (a series with ~50k committed samples) in ws-A:

1. The client calls `series.watch` (motion). The host authorizes `mcp:series.watch:call` in ws-A, subscribes
   the Zenoh subject, and the client records the first live sample's `seq` as the **backfill boundary** `B`.
2. The client calls `series.read { series:"floor2/temp", limit:500, direction:"back", mode:"rows" }` with no
   cursor (page 1, history older than `B`). The host authorizes `mcp:series.read:call` **workspace-first** in
   ws-A, resolves ws/series from the token, and issues the keyset seek
   `WHERE (series, seq) < (…, B) ORDER BY seq DESC LIMIT 500` against the indexed series-plane table.
3. The host returns `{ rows:[500 samples], next_cursor:"<opaque>", prev_cursor:"<opaque>" }`. `next_cursor`
   encodes the oldest returned row's `(series, seq)` (slice A's codec). The chart renders 500 points; the
   live watch keeps appending at the head — no overlap, because history is strictly `< B`.
4. The user scrolls; the client echoes `next_cursor` back: `series.read { limit:500, cursor:"<opaque>",
   direction:"back" }`. The host **re-authorizes** in ws-A (cursor grants nothing), decodes the position key
   via slice A, and seeks the next 500 older rows. **Page-500 latency ≈ page-1 latency** — the seek is
   O(page), not O(offset).
5. Paging reaches the start of the series: the host returns `{ rows:[…], next_cursor:null }`.
   `next_cursor == null` means end-of-range; the client stops "load more." Every one of the ~50k samples was
   rendered exactly once — history via paging, the newest via the watch, meeting at `B`.

**Deny variant:** mid-scroll, an admin revokes ws-A's `mcp:series.read:call`. The next `series.read` page —
even with a valid `next_cursor` — is denied at `authorize_ingest`; the cursor is not a bypass. **Isolation
variant:** a ws-B token replays ws-A's `next_cursor`; the host resolves ws-B from the token, seeks ws-B's
namespace, and returns empty (the ws-A position key resolves nothing in ws-B).

## Testing plan

Per `scope/testing/testing-scope.md` — real `mem://` SurrealDB **seeded with a large real series (tens of
thousands of samples**, so keyset visibly beats offset), real host cap checks, real gateway. No mocks; no
`*.fake.ts`; seed real records into the real store.

- **Capability-deny (mandatory):** `series.read` is denied without `mcp:series.read:call`, and a denied
  caller learns nothing (empty/deny, no leak). **Grant revoked mid-chain** denies the *next* page even with a
  valid `next_cursor` — proving the cursor is a bookmark, not a capability bypass.
- **Workspace-isolation (mandatory):** ws-B replaying a ws-A `next_cursor` gets empty/deny; the seek runs in
  ws-B's namespace and the ws-A position key resolves nothing. A ws-B caller can never observe a ws-A sample.
- **Keyset correctness:** walking the full chain returns **every seeded sample exactly once, in order, no
  gaps, no dupes**. Includes: (a) a **tie on the sort key** — samples sharing a `ts` are disambiguated by
  `seq` and never skipped/repeated; (b) **concurrent head-appends** during a backward chain leave already-
  returned older pages unaffected (append-stable); (c) `next_cursor == null` **exactly** at end-of-range,
  never one page early or late; (d) the `series.watch` boundary compose — every sample appears once across
  watch + paging, none twice at the seam.
- **Performance (the regression that proves the feature):** **page-1 latency ≈ page-500 latency within a
  band** and **bounded memory** per page, measured against a **control `OFFSET` read that degrades** with
  depth on the same seeded series. A plan/`.explain()` assertion proves the keyset **seeks the index**, not
  scans — this is what pins the "fast page loads" promise and the index guarantee.
- **Bound enforcement:** `limit` above the max is clamped (not honored); a missing `limit` uses the default;
  a malformed/incompatible cursor is rejected cleanly (restart-the-chain), never mis-seeks (slice A's
  versioning, exercised through this verb).

## Risks & hard problems

- **The index must actually be used.** If SurrealDB plans a full-series scan instead of the `(series, seq)`
  seek — a missing index, a predicate shape it won't push to the index — the fast path silently becomes
  O(series) and only a large-series performance test catches it. The `.explain()`/plan assertion is
  load-bearing, not decoration.
- **The watch/page seam.** Getting the backfill boundary `B` wrong double-renders (overlap) or drops (gap)
  samples at the join. The boundary must be the newest committed `seq` at subscribe time and paging strictly
  older than it; append-stability is what makes this safe under concurrent ingest — a property to test, not
  assume.
- **Tie discipline is slice A's but breaks *here* if the seek key isn't the unique composite.** If this verb
  seeks on `ts` alone (not `(ts, seq)`) for the time-cursor variant, ties silently skip/duplicate rows.
  This slice must always seek the unique composite.
- **Default/max `limit` shape.** Too high re-opens the OOM this fixes; too low makes the dashboard chatty.
  The bound is a real decision (open question), and "no limit" must be impossible — the unbounded return is
  the concrete gap being closed.

## Open questions

- **Direction support at v1:** `direction:"back"` only (the dashboard need) first, with `prev_cursor` echoed
  for later, vs full bidirectional `fwd`/`back` from day one? (Parent leans back-first.)
- **Default and max `limit`** for `series.read` rows mode — the numbers, and the client's page-count ceiling
  before it must offer "export as a job" (§6.10). Coordinate with slice E.
- **Time-cursor vs seq-cursor default:** page by `(series, seq)` (commit order, always unique) by default and
  offer `(series, ts, seq)` only when a caller pages by wall-clock — or always time? Affects which index is
  the primary guarantee.
- **`next_cursor` at a live head:** when back-paging catches up to the watch boundary `B`, does the host
  return `next_cursor` pointing at `B` (client re-anchors) or signal "you've reached the live edge"
  distinctly? Affects the seam contract with slice E.

## Related

- [`page-chaining-scope.md`](page-chaining-scope.md) — **parent**: the doctrine + the one shared contract
  this slice obeys.
- [`page-cursor-scope.md`](page-cursor-scope.md) — **slice A**, the foundation this **consumes**: the opaque
  cursor codec + keyset predicate + tiebreaker discipline + cursor versioning.
- [`series-decimation-scope.md`](series-decimation-scope.md) — **slice C**, `mode:"buckets"`, builds on this.
- [`federation-paging-scope.md`](federation-paging-scope.md) — **slice D**, external/federated paging.
- [`page-chaining-ui-scope.md`](page-chaining-ui-scope.md) — **slice E**, a caller: composes this backward
  paging with the live `series.watch` tail.
- [`../ingest/ingest-scope.md`](../ingest/ingest-scope.md) — the `series` plane + `series.read`/`latest`/`watch`.
- [`ingest/read.rs`](../../../rust/crates/host/src/ingest/read.rs) — the code this slice modifies
  (`series_read_range`).
- README `§3` (rules 2/3/5/6), `§6.1` (API shape).
