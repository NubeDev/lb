# Datasources scope — page-chaining UI (the two frontend callers)

Status: scope (the ask) — child **slice E** of [`page-chaining-scope.md`](page-chaining-scope.md). Promotes
to `public/datasources/datasources.md` once shipped. Depends on the shipped verbs from slices **B**
([`series-paging-scope.md`](series-paging-scope.md), rows), **C**
([`series-decimation-scope.md`](series-decimation-scope.md), buckets), and **D**
([`federation-paging-scope.md`](federation-paging-scope.md), federated). **Pure client work over the
gateway** — no host change, no new cap, no new verb.

The two callers that actually page a large series need to do it well: the **data-console table** loads raw
rows a window at a time and "load more" to chain backward; a **dashboard viz cell** loads a decimated first
window and pans back through history by time cursor. Both must **compose backward paging with the live
`series.watch` tail** — subscribe forward for new samples at the right edge, page backward to backfill
history — without double-rendering the sample where the two meet. This slice is the React clients only; it
consumes the shared contract slices A–D built and never invents backend behavior.

> Read with: [`page-chaining-scope.md`](page-chaining-scope.md) (the parent doctrine + the one shared
> contract — the client obeys it, doesn't extend it), [`series-paging-scope.md`](series-paging-scope.md)
> (`series.read` rows source), [`series-decimation-scope.md`](series-decimation-scope.md) (`series.read`
> buckets source), [`federation-paging-scope.md`](federation-paging-scope.md) (federated paged source),
> [`../frontend/data-console-scope.md`](../frontend/data-console-scope.md) (the table caller),
> [`../frontend/dashboard/`](../frontend/dashboard/) + [`../frontend/dashboard/viz/`](../frontend/dashboard/viz/)
> (the chart caller), [`../ingest/ingest-scope.md`](../ingest/ingest-scope.md) (`series.watch`, the live
> tail). README `§3` (rule 3 state-vs-motion), `§6.1` (API shape).

## Goals

- **A `usePagedRows` hook + a "load more" table** (the data-console rows caller). The table calls
  `series.read({mode:"rows", limit, cursor, direction:"back"})`, renders the window, and on scroll-to-bottom
  / "Load more" **echoes `next_cursor`** to fetch the next older window and appends it. Infinite scroll, not
  a numbered pager. `next_cursor == null` disables the control (end-of-range reached).
- **A `usePagedBuckets` hook + a pan-back chart cell** (the dashboard viz caller). The cell loads a
  **decimated first window** (`mode:"buckets"`, a bounded point budget from slice C), and **panning left**
  chains the **time cursor** backward one window at a time, prepending the older buckets to the series it
  draws. Zoom/pan is window-chaining, not a re-scan of the whole range.
- **Compose backward paging with the live `series.watch` tail without double-rendering the seam.** The right
  edge is a live forward `series.watch` subscription (new samples arrive as motion, Zenoh); the left is
  backward keyset paging (committed state, SurrealDB). The client owns the join: dedup by the sample's
  unique composite key (`(ts, seq)`) so the newest paged sample and the oldest live sample **never both
  render** at the seam.
- **A client page-count ceiling → "export as a job."** After **N** "load more"s (default proposal below),
  the client stops offering more paging and instead offers **"export this range as a job"**
  (`federation.mirror` / an export `lb-jobs` job). Paging serves an *interactive window*; a whole-range pull
  is a durable job that returns a job id (README §6.10), never a client loop of thousands of `next_cursor`s.
- **Honest deny + honest end-of-range.** A caller lacking the read cap surfaces the `Denied` honestly
  (inline error, not a blank grid / empty chart). `next_cursor == null` is "you've reached the start of the
  range," rendered as such, distinct from an error.

## Non-goals (the defer-list)

- **No host / backend paging logic.** The cursor codec (slice A), the keyset predicate + index guarantee
  (B), decimation math (C), and pushdown detection (D) are **owned by A–D and only referenced here.** The
  client never constructs, inspects, decodes, or validates a cursor — it is an opaque token echoed as-is.
- **No new capability, no new verb, no new gateway route.** This slice consumes the existing gated verbs
  through the caller's existing cap over the gateway. If a verb isn't reachable over the gateway yet, that
  gap belongs to B/C/D, not here.
- **No numbered pager, no total count.** "Page 42 of 1000" and counting a giant series are feature-level
  non-goals (parent). The table is "load more"; the chart is "pan back."
- **No client-side bulk export loop.** Past the page-count ceiling the answer is a **job**, not a hidden
  loop that pages until it OOMs the tab. Paging is not export.
- **No client-side decimation.** The chart draws server-decimated buckets (slice C); it does not fetch raw
  points and thin them in the browser (that defeats the point budget).
- **No cursor persistence as a bookmark/scope.** A cursor is a transient in-memory position for *this*
  chained view, not a shareable/stored scope — the workspace wall is the token, not the cursor (parent
  contract).

## Intent / approach

**Two thin hooks over one contract; the tricky part is the seam, not the paging.** The paging itself is
almost mechanical: hold a `cursor` in component state, call the verb, append/prepend the window, store the
returned `next_cursor`, stop when it's `null`. Each caller gets **one hook that owns the chain state** and a
**presentational component that owns rendering** — one responsibility per `.tsx` file (FILE-LAYOUT), no
shared `paging-utils.ts`.

- `usePagedRows` (data-console): owns `{ rows, cursor, atStart, loadCount }`; `loadMore()` calls
  `series.read` rows and **appends**; surfaces `denied`/`error`/`atStart`. The table `.tsx` is
  presentational — it renders rows and calls `loadMore()` on the intersection-observer / button.
- `usePagedBuckets` (dashboard viz cell): owns the decimated window + a `panBack()` that chains the **time
  cursor** and **prepends** older buckets; it reuses the cell's existing `DataSourceRef` → `(tool, args)`
  resolution ([`../frontend/dashboard/viz/`](../frontend/dashboard/viz/)) — the ref just carries
  `mode:"buckets"` + `limit`, and paging is `cursor` on the same target.

**The seam is the real design problem — and the decision.** New samples enter at the right (`series.watch`,
motion); backfill enters at the left (backward keyset paging, state). They are two independent streams that
meet at "now-ish." The naive client renders both and the boundary sample appears twice (once from the last
paged window, once from the first live event), or a sample slips through the gap between "last committed
page" and "first live event." **Decision: the client keys every rendered sample by its unique composite key
`(ts, seq)` (the same tiebreaker slice A defines) and merges into a single ordered, de-duplicated view.**
The live tail is authoritative for the right edge; paged windows fill leftward; the merge drops any
`(ts, seq)` already present. This is the client half of rule 3 (state vs motion): *subscribe forward, page
backward, join by key* — never conflate the watch feed with a page, never page the live edge.

**The page-count ceiling is a doctrine boundary, not a UX nicety.** Paging is O(page) and flat-latency, but
a human dragging "load more" 5,000 times is doing **bulk export by hand** — the thing the parent explicitly
routes to a job. So the client caps interactive paging at **N windows** and, at the ceiling, swaps the
"load more" affordance for **"export this range as a job"** (`federation.mirror` / an export `lb-jobs` job,
returns a job id the user watches). This keeps the interactive path fast and the bulk path durable/resumable.

**Rejected alternatives:**
- *Merge the live tail and the paged history by array-concat / timestamp only.* Rejected — timestamp ties
  (two samples same `ts`) double-render or drop at the seam; the `(ts, seq)` composite key is the only
  correct dedup key, and it's the one slice A already guarantees is unique.
- *Let the chart fetch raw points and decimate client-side on pan.* Rejected — reintroduces the unbounded
  payload slice C exists to prevent; the browser must draw buckets, not thin a firehose.
- *Endless `next_cursor` looping for "download everything."* Rejected — that's export, and export is a job
  (parent non-goal). The ceiling makes the boundary explicit instead of letting the UI degrade silently.
- *Store the cursor in the URL / a saved view as a resumable bookmark.* Rejected for this slice — a cursor
  is a transient position, not a scope; a stale cursor across a schema/order change is slice A's "restart
  the chain" case, not a durable link. (A saved *time range* is fine; a saved *cursor* is not.)

## How it fits the core

- **Tenancy / isolation:** the client always operates within the **token's workspace** — every paged read
  goes through the gateway session, which binds the ws from the token, not from any request field or the
  cursor. **A cursor is not a scope:** a ws-A cursor replayed in a ws-B session resolves nothing (the host
  re-authorizes ws-first per page — parent contract). Mandatory isolation test: a ws-B session cannot page
  ws-A's series.
- **Capabilities:** **no new cap.** The table pages under the caller's existing `mcp:series.read:call`; the
  chart cell under whatever its `DataSourceRef` already resolves to (`mcp:series.read:call` /
  `mcp:federation.query:call` / `mcp:query.run:call`). A mid-chain revoke denies the *next* page (host
  re-checks per page); the client surfaces that `Denied` honestly. **Mandatory deny-test:** a caller
  lacking the read cap sees the deny surfaced, not a blank.
- **Placement:** `either` — pure gateway client work; the browser reaches the verbs over HTTP/SSE, the
  desktop shell over the same MCP contract. No `if cloud {…}`.
- **MCP surface** (walk all four — SCOPE-WRITTING §6.1; this slice *consumes*, exposes nothing new):
  - **Get / list:** consumes `series.read` (`mode:"rows"` for the table, `mode:"buckets"` for the chart —
    slices B/C) and the federated paged reads `federation.query` / `query.run` (slice D). All additive
    `{limit, cursor, direction}` on existing reads; the client sends them and echoes `next_cursor`.
  - **Live feed (SSE / watch):** consumes the **existing** `series.watch`
    ([`../ingest/ingest-scope.md`](../ingest/ingest-scope.md)) for the forward tail — the client subscribes,
    it does not add a new feed. The seam-merge is where watch composes with the paged reads.
  - **CRUD (write):** **N/A** — read-only callers.
  - **Batch:** the page-count ceiling hands off to **`federation.mirror` / an export `lb-jobs` job** (a
    long batch is a job that returns a job id — README §6.10); the client *offers* and *watches* it, it does
    not implement it.
- **Data (SurrealDB) / Bus (Zenoh):** the client touches neither directly. Paged windows are committed state
  (SurrealDB, via the verb); the tail is motion (Zenoh, via `series.watch`). Rule 3 lives in the seam-merge:
  keep them separate, join by key on the client, never use one as the other.
- **State vs motion (rule 3):** the whole slice is the client-side embodiment of it — backward through
  committed state, forward on the live bus, joined by `(ts, seq)`.
- **One responsibility per file:** `usePagedRows.ts` / `usePagedBuckets.ts` (chain state), the table `.tsx`
  and the chart cell `.tsx` (presentation), a `mergeTail.ts` (the seam dedup) — each one verb, no
  `paging-utils.ts`.
- **SDK/WIT impact:** none — no boundary change; the client sends already-additive fields.

## Example flow

**Data-console table ("load more" over rows):**
1. A user opens the **Data** page series view. `usePagedRows` calls
   `series.read({series, mode:"rows", limit:50, cursor:null, direction:"back"})` → 50 newest rows +
   `next_cursor`. The grid renders them newest-first.
2. They scroll to the bottom (or click **Load more**). The hook calls the same verb with `cursor: next_cursor`
   → the next 50 older rows + a new `next_cursor`; the hook **appends** and updates the cursor.
3. They keep going. When a page returns `next_cursor == null`, the hook sets `atStart` and the "Load more"
   control is replaced by an "**Start of range**" marker — an honest end, not an empty page.
4. If a mid-chain revoke lands, the next `series.read` returns `Denied`; the hook surfaces it as an inline
   error at the foot of the grid (rows already shown stay), not a blank grid.

**Dashboard viz cell (pan back over buckets, composed with the live tail):**
1. A chart cell mounts. `usePagedBuckets` calls `series.read({series, mode:"buckets", limit:<point budget>,
   cursor:null, direction:"back"})` → a decimated first window (min/max/avg/last buckets, slice C) + a time
   `next_cursor`. In parallel it subscribes `series.watch(series)` for the forward tail.
2. New samples arrive on the watch feed at the right edge; `mergeTail` inserts each by `(ts, seq)`, dropping
   any key already present from the first window — **the seam sample renders exactly once.**
3. The user **pans left** past the loaded window. `panBack()` calls `series.read` with `cursor: next_cursor`
   → the next older decimated window; the hook **prepends** the buckets and updates the time cursor.
4. After **N** pan-backs (the ceiling), the cell stops offering more paging and shows **"Export this range
   as a job"**; clicking it enqueues `federation.mirror` / an export `lb-jobs` job and shows the returned
   job id + status, instead of looping `next_cursor` to bulk-download in the tab.

## Testing plan

**No mocks / no fake backend (CLAUDE §9, `../testing/`-style §0) — this is the load-bearing rule for a UI
slice.** Every test runs against a **real spawned gateway node** with a **real seeded series** over the
bridge — `pnpm test:gateway`, `*.gateway.test.tsx`, per the `vitest.gateway.config.ts` harness. Seed real
samples through the real `ingest.write` path (never a hand-built array); page them through the real
`series.read`/`federation.query` verbs over the real gateway. **A `*.fake.ts` / any hand-written
re-implementation of node paging is banned** — it would let the seam bug hide behind a fake that always
"merges cleanly."

Mandatory categories from the testing scope that apply:

- **Capability deny (mandatory):** a `*.gateway.test.tsx` where the session lacks the read cap — the table
  and the chart cell **surface the `Denied` honestly** (inline error), **not a blank grid / empty chart**.
  A mid-chain revoke denies the next page and is surfaced; already-rendered windows remain.
- **Workspace isolation (mandatory):** a **ws-B session cannot page ws-A's series** — seed a series in ws-A,
  attempt to page it from a ws-B token; the paged reads resolve nothing (the wall is the token, the cursor
  is not a scope). Attempting to replay a ws-A `next_cursor` in the ws-B session still resolves nothing.
- **Table "load more" chains correctly over the real gateway:** seed M real samples, page in windows of
  `limit`, assert every sample appears **exactly once across windows** (no skip/dupe at page boundaries —
  the `(ts, seq)` tiebreaker), and that `next_cursor == null` arrives exactly at the start of the range and
  disables the control.
- **Chart pans back by time cursor:** seed a wide time range, load the decimated first window, `panBack()`
  N times, assert each older window prepends and the buckets are contiguous with no gap/overlap at the
  window seams.
- **Live watch composes with backward paging without duplicating the seam sample (the headline case):** seed
  history, subscribe `series.watch`, then `ingest.write` a new sample at the live edge; assert the sample
  the paged window and the watch feed share renders **exactly once** (dedup by `(ts, seq)`), and no sample
  is dropped in the gap between the last committed page and the first live event.
- **Page-count ceiling:** after N `loadMore()`/`panBack()`s, assert the client offers "export as a job"
  (enqueues `federation.mirror` / an export `lb-jobs` job, shows a job id) instead of a further paged read.
- **Offline/sync, hot-reload:** **N/A** — read-only transient client view; nothing durable or stateful added.

## Risks & hard problems

- **The seam double-render (the headline risk).** Joining the live `series.watch` tail with backward paging
  is the one genuinely hard bug: dedup **must** key on the unique composite `(ts, seq)`, not on `ts` alone
  (ties double-render or drop) and not on array position (a live insert shifts it). Get the merge wrong and
  a user sees a phantom duplicate sample — or a missing one — right at "now." Regression test is mandatory.
- **Gap at the seam.** Symmetric to the double-render: between "last committed page" and "first live event"
  a sample can slip through if the client starts the live subscription *after* fetching the first page
  without covering the overlap. Subscribe first (or overlap the first page with the live edge) and let the
  key-dedup absorb the overlap — never leave an uncovered instant.
- **Cursor invalidation mid-chain.** A schema/order change (or a cursor-version bump, slice A) can make a
  held cursor un-seekable; the host rejects it cleanly ("restart the chain"). The client must handle that as
  *reset to the first window*, not a hard error or a mis-seek — easy to forget until it happens live.
- **Ceiling tuning.** Too low and interactive back-scroll feels crippled; too high and the UI becomes a
  bulk-export loop the parent forbids. N is an open question below; make it a config constant, not a magic
  number scattered across two hooks.
- **Two callers, one temptation to share.** The rows hook and the buckets hook are similar enough to invite a
  `paging-utils.ts`; resist it (FILE-LAYOUT — no `utils.ts`). The *contract* is shared (slices A–D); the
  client code is two small, honest hooks.

## Open questions

- **Page-count ceiling N** (per caller): a first proposal is **~20 windows for the table** and **~10 pan-back
  windows for the chart** before offering the export job — confirm against the default/max `limit` slices
  B/C settle, so N × limit is a sane interactive budget. (Feeds the parent's "page-count ceiling" open
  question.)
- **Direction support:** ship **`direction:"back"` only** (the table load-more + chart pan-back need) first,
  or wire `prev_cursor`/forward paging (jump forward after panning back) from day one? (Tracks slices B/E in
  the parent.)
- **Export handoff target:** at the ceiling, is the offered job **`federation.mirror`** (mirror the range
  into the series plane) or a dedicated **export `lb-jobs`** job (download an artifact)? — depends on which
  slice D / jobs ships; the UI offers whichever exists.
- **Seam authority:** is the **live `series.watch` feed** always authoritative for the overlapping edge, or
  does the last committed page win on conflict? (Recommend watch-wins for the right edge; confirm no case
  where a page carries a *newer* committed value than the tail.)
- **Chart pan UX:** does panning left auto-`panBack()` at the window edge, or require an explicit "load
  earlier" gesture? (Auto is smoother; explicit is cheaper and makes the ceiling legible.)

## Related

- [`page-chaining-scope.md`](page-chaining-scope.md) — **parent**: the doctrine + the one shared contract
  this client obeys (opaque cursor, `next_cursor` chaining, two modes, ws-is-the-wall).
- [`page-cursor-scope.md`](page-cursor-scope.md) — **slice A**: the opaque cursor codec + the `(ts, seq)`
  tiebreaker the client dedups the seam by (never constructs).
- [`series-paging-scope.md`](series-paging-scope.md) — **slice B**: the `series.read` **rows** source the
  table pages.
- [`series-decimation-scope.md`](series-decimation-scope.md) — **slice C**: the `series.read` **buckets**
  source the chart pans.
- [`federation-paging-scope.md`](federation-paging-scope.md) — **slice D**: the federated paged source
  (`federation.query` / `query.run`) a cell can also page.
- [`../frontend/data-console-scope.md`](../frontend/data-console-scope.md) — the table caller (the raw
  row grid this adds "load more" to).
- [`../frontend/dashboard/`](../frontend/dashboard/) + [`../frontend/dashboard/viz/`](../frontend/dashboard/viz/)
  — the chart caller (the viz cell + its `DataSourceRef` → `(tool, args)` resolution this pages).
- [`../ingest/ingest-scope.md`](../ingest/ingest-scope.md) — `series.watch`, the live forward tail the seam
  composes with.
- README **`§3`** (rule 3, state vs motion), **`§6.1`** (API shape), **`§6.10`** (jobs — the export handoff).
</content>
</invoke>
