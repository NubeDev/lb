# Datasources scope — page cursor + keyset predicate (slice A of page-chaining)

Status: scope (the ask). Promotes to `public/datasources/datasources.md` once shipped.

The keystone of [page-chaining](page-chaining-scope.md): the **opaque cursor codec** and the **keyset
predicate builder** that every pager reuses. A read gets `{limit, cursor, direction}`; the cursor encodes
the last row's *unique composite position key*; the builder turns it into a `WHERE key ⋛ cursor ORDER BY
key LIMIT n+1` — an index seek, not an `OFFSET` scan. This slice ships the pure primitive and its contract:
two files, no store wiring. Slices B/C/D consume it; none of them re-derives the cursor shape or the
tiebreaker rule. The whole feature is O(page)-fast or wrong right here — the tiebreaker discipline lives in
this slice and nowhere else.

## Goals

- **One opaque cursor.** `encode(position_key, direction, mode, version) -> String` and its total-function
  inverse. Base64 of a compact, self-describing payload. Callers treat it as a bookmark and echo it back;
  they never construct or parse one.
- **One keyset predicate builder.** `(cursor?, limit, direction) -> KeysetQuery` yielding the comparison
  clause, `ORDER BY key` in the requested direction, and `LIMIT limit+1`. The `+1` is the whole trick:
  fetching one extra row tells the pager whether a `next_cursor` exists **without a `COUNT`** — pop the
  extra, page it to `next_cursor`, return exactly `limit` rows.
- **Tiebreaker discipline, owned here.** The position key is a **UNIQUE composite** (`(ts, seq)` /
  `(series, seq)`), never a lone non-unique column. The builder makes the composite comparison the *only*
  seek shape it can emit — a lexicographic `(a, b) < (c, d)` — so a caller physically cannot ask for a
  skip/dupe-prone single-column seek.
- **Clean cursor versioning.** A `version` (and `mode`/direction) tag rides inside the cursor. Decode of an
  **incompatible** cursor returns a typed "stale cursor — restart the chain" result, never a mis-seek and
  never a hard 500.
- **Resolve the encoding question (feature open question, parent §Open questions).** Plain base64, **no
  HMAC** — recommended and defaulted here.
- **Reusable across pagers.** The same two functions serve `series.read` (B), decimated buckets (C), and
  federated/store keyset (D). No pager-specific logic leaks in.

## Non-goals

- **No `series.read` / `store.query` / `federation.query` wiring** — that is slices B and D. This slice has
  no MCP verb, no capability, no gateway route; it is called *by* those.
- **No decimation / bucketing** — slice C. `mode:"buckets"` is a tag this codec carries opaquely; the
  bucketing math is C's.
- **No pushdown detection / connector logic** — slice D. This builder emits an abstract keyset query;
  translating it to a specific engine's SQL/pushdown is the consumer's job.
- **No `prev_cursor` semantics beyond direction** — the codec carries `direction`; whether a read exposes
  bidirectional paging is decided in B/E. This slice makes both directions *encodable*, nothing more.
- **No offset paging, no total counts** — rejected feature-wide (parent §Non-goals).
- **No new store, table, or index** — an index guarantee on the paging key is slice B's assertion, not a
  new persistence layer.

## Intent / approach

**A cursor is a serialized seek position, not a query and not a grant.** Encode the minimum a next page
needs to resume: the position key (the last emitted row's unique composite), the sort direction, the read
`mode`, and a format `version`. Base64 makes it URL/JSON-safe and opaque enough that clients don't
hand-edit it; that's the *only* property we need from the encoding, because **the cursor authorizes
nothing** — see below.

The keyset builder is the sole constructor of a paging query. Given `direction:"back"` it emits `WHERE key
< cursor ORDER BY key DESC LIMIT n+1`; forward flips the comparator and order. With **no** cursor it emits
just `ORDER BY key <dir> LIMIT n+1` — the first page. Because the key is a composite, the comparison is
lexicographic tuple compare, which the store executes as an index range seek: **O(page), flat latency at
any depth** — the property the whole feature exists for.

**Encoding decision — plain base64, no HMAC (resolves the parent's open question).** The cursor is a
bookmark, not a capability (shared contract). Every page **re-authorizes** workspace-first then the read
cap, and the workspace + series come from the **token/request**, never the cursor. So a tampered or forged
cursor grants *nothing*: at worst it names a position **inside data the caller may already read**, and the
re-auth + version check catch a malformed one. Signing would add key management and a verify hop to protect
a value that confers no authority — cost with no security return. **Rejected — HMAC-signed cursors** for
security; noted only as a future option if **audit tamper-evidence** (proving a client didn't hand-craft a
position) is ever wanted — that is provenance, not access control, and can be added as a `version` bump
without touching consumers.

**Rejected — encode the workspace/series in the cursor** (so a page "knows" its scope). This is the classic
keyset footgun: it invites trusting the cursor for authority and turns a replayed cursor into a
cross-tenant leak. We deliberately keep ws/series **out** of the cursor; they are re-supplied from the
token every page. A cursor is scope-free by construction.

**Crate placement (FILE-LAYOUT).** Two files, one responsibility each, ≤400 lines:
- `cursor` — encode/decode the opaque bookmark (base64, version/mode/direction tag, typed stale-cursor
  result).
- `keyset` — turn `(cursor?, limit, direction)` into the keyset query (comparator + order + `limit+1`).

**Recommended home: [`lb-store`](../../../rust/crates/store/src/lib.rs)** — the shared substrate every pager
sits on. `lb-ingest` reads (B, via [`ingest/read.rs`](../../../rust/crates/ingest/src/read.rs)) and
`store.query` / federation (D) both depend on `lb-store` already, so a `store::keyset` primitive is reusable
without a new dependency edge and without pulling series semantics into the store. **Open question flagged**
below in case the composite-key shape proves ingest-specific enough to want it in `lb-ingest` instead.

## How it fits the core

- **Tenancy / isolation (the hard wall):** the cursor **carries no workspace and no series** — by design.
  Every page re-derives them from the token; the codec has no field for them and the builder never reads
  them from a cursor. A ws-A cursor decoded under a ws-B token yields a *position* only; the store query is
  namespace-scoped by the caller (as [`ingest/read.rs`](../../../rust/crates/host/src/ingest/read.rs)
  already is), so it resolves within ws-B or resolves nothing. The cursor cannot move data across the wall.
- **Capabilities:** **no new capability, no new verb** — this is a library primitive below the MCP surface.
  It is unreachable except through a consuming verb that already gates (`mcp:series.read:call`, etc.),
  re-checked per page by that verb. The deny path is unchanged and owned by the consumer.
- **Placement:** `either`, pure logic — no `if cloud`. The same codec/builder pages a local edge series and
  a mirrored cloud table; which sources exist is config, not a branch.
- **MCP surface (§6.1):** **none directly.** No CRUD, no get/list, no live-feed, no batch. It is consumed by
  the get/list reads that slices B/D extend with `{limit, cursor}`. Stated explicitly so no verb is
  invented here.
- **Data (SurrealDB):** no table, no record, no new index. It **produces** a keyset predicate the consumer
  runs against an existing index; it stores nothing. State vs motion: purely on the **state** (committed,
  backward-paged) side — it never touches the bus.
- **Bus (Zenoh):** N/A — the live forward edge is `series.watch`; this primitive walks committed state only.
- **Sync / authority:** N/A — stateless pure functions; a cursor is client-held, node-agnostic. Any node
  that can read the series can resume any cursor for it.
- **Secrets:** none. The no-HMAC decision means **no key material** — nothing to mediate, rotate, or leak.
- **SDK/WIT impact:** none in this slice — the primitive is host-internal. The additive `{limit, cursor}`
  fields land on MCP verbs in B/D. Flagged so B/D own the (additive, non-breaking) boundary change.

## Example flow

A dashboard pages backward through a large series; this slice is the two calls the consuming read makes.

1. **First page.** Consumer (slice B) calls `keyset::build(cursor=None, limit=100, direction=Back)`. It gets
   back `ORDER BY (series, seq) DESC LIMIT 101`. It runs that (namespace-scoped by the ws-B token) and gets
   ≤101 rows.
2. **Detect more + mint the cursor.** 101 rows came back, so there *is* a next page. Consumer drops the
   101st, keeps 100, and calls `cursor::encode(position = (series, seq) of the 100th row, direction=Back,
   mode=Rows, version=1)`. It returns `{rows: 100, next_cursor: "<base64>", prev_cursor: null}`.
3. **Next page.** Client echoes `next_cursor` back. `keyset::build(cursor=<decoded>, limit=100,
   direction=Back)` emits `WHERE (series, seq) < (s, 900) ORDER BY (series, seq) DESC LIMIT 101` — an index
   seek straight to position 900, **not** a scan of the first 100.
4. **Tie safety.** Two rows share a `ts`; because the key is the unique composite `(series, seq)`, the seek
   lands *between* them deterministically — no row skipped, none repeated, even as new samples append at the
   head (they sort *above* the cursor and never shift the page).
5. **End of range.** A page returns ≤100 rows (no 101st): consumer sets `next_cursor: null`. Chain done.
6. **Stale cursor.** A month later the client resumes a `version:1` cursor after a `version:2` key-shape
   ships. `cursor::decode` returns `StaleCursor` (not an error, not a mis-seek); the consumer restarts the
   chain from page one. The client sees a clean reload, never wrong data.

## Testing plan

Per [`../testing/testing-scope.md`](../testing/testing-scope.md) — real `mem://` store, real seeded
records, no mocks (CLAUDE §9). Mandatory categories that apply, plus the primitive's own cases:

- **Round-trip (unit):** `decode(encode(x)) == x` across every field (position, direction, mode, version);
  malformed/truncated base64 decodes to a typed error, never a panic.
- **Keyset beats offset (integration, the headline proof):** seed a **large** series (e.g. 1M samples) into
  the real `mem://` store; page it to the end with the keyset builder and assert deep-page latency is
  **flat** (page 10 000 ≈ page 1), then contrast with an `OFFSET` read whose latency grows with depth.
  Proves O(page).
- **No-skip / no-dupe across a tie (integration, owns the headline risk):** seed rows that **collide on
  `ts`** and page across the collision; assert the full set is returned exactly once, in order, with the
  composite tiebreaker. Then repeat with a single-column key to *demonstrate the skip/dupe* the composite
  prevents (a guard test that the discipline is load-bearing).
- **No-skip under concurrent head-appends (integration):** page backward while a writer appends new samples
  at the head; assert the backward page set is unaffected (append-stability) — no row seen twice, none
  missed.
- **Cursor versioning (unit + integration):** an incompatible `version` decodes to `StaleCursor`; the
  consumer restarts cleanly and returns correct page-one data — never a mis-seek into the wrong position.
- **Workspace-isolation (MANDATORY):** a cursor minted while reading ws-A, replayed under a ws-B token,
  resolves only within ws-B (or nothing) — the store query is namespace-scoped by the token, not the
  cursor. Assert the cursor supplies **no** ws authority.
- **Capability-deny (MANDATORY, at the seam):** since this primitive has no capability of its own, the test
  asserts the **negative**: a **tampered or foreign cursor carries no read authority** — the consuming read
  still gates on its cap, and a hand-edited cursor at worst names a position **within what the caller may
  already read**, never across the wall. (The consuming verb's own deny-test lives in B/D; this asserts the
  cursor can't bypass it.)
- **Fuzz (unit):** random/adversarial cursor strings decode to `StaleCursor`/error, never a panic and never
  a silently-wrong seek.

## Risks & hard problems

- **Tiebreaker discipline is the whole feature (README §11 altitude).** A keyset on a non-unique key
  silently skips or repeats rows on ties — the single easiest thing to get wrong, and invisible until a
  user notices a missing row. Mitigation: the builder makes the **unique composite** the only expressible
  seek; a single-column key is not an option the API offers. The guard test above proves the composite is
  load-bearing.
- **Trusting the cursor for scope.** The tempting shortcut — read ws/series from the cursor to "save a
  lookup" — is a cross-tenant leak. Mitigation: the cursor has **no field** for them; the codec structurally
  can't leak scope. Enforced by the isolation test.
- **`u64::MAX` / float-coercion footgun.** Encoding an open bound as a sentinel repeats
  [`debugging/ingest/u64-max-bound-coerces-to-float.md`](../../debugging/ingest/u64-max-bound-coerces-to-float.md):
  a near-`2^64` int coerces to float and the comparison mis-evaluates to empty. Mitigation: a first page is
  a **cursor-absent** build (omit the clause), never a `MAX` sentinel.
- **Silent version drift.** A cursor outliving a key-shape change could mis-seek if `version` isn't checked
  before use. Mitigation: `decode` checks `version` **first** and returns `StaleCursor` — restart, never
  guess.
- **Encoding lock-in.** Base64-of-payload is a wire format clients persist; a sloppy layout is hard to
  evolve. Mitigation: `version` is the first field, so the format is bumpable without breaking consumers
  (old cursors go `StaleCursor`).

## Open questions

- **Crate home — `lb-store` (recommended) vs `lb-ingest`?** `lb-store` maximizes reuse (D reads from the
  store directly, not through ingest). Decide during implementation: if the composite-key shape ends up
  ingest-specific, `lb-ingest` may be the cleaner owner. Either way it's the same two files.
- **Payload layout inside the base64.** A tagged binary (compact, opaque) vs. a small JSON blob (readable in
  debug logs). Recommend compact binary with `version` first; JSON is a fallback if debuggability wins.
- **Position-key representation for a `(ts, seq)` composite.** Fixed-width big-endian tuple so lexicographic
  byte-compare matches numeric compare, vs. delimited fields. Recommend fixed-width to sidestep delimiter
  escaping and preserve ordering.
- **`mode` field ownership.** Confirm with slice C that a single `mode` tag (`rows`/`buckets`) is enough, or
  whether buckets need a bucket-width in the cursor (that would make it C's field, carried opaquely here).
- **HMAC later?** Left open per the parent: add signed cursors **only** if audit tamper-evidence is wanted;
  it's a `version` bump, invisible to consumers. Not needed for security.

## Related

- [`page-chaining-scope.md`](page-chaining-scope.md) — **parent**: the doctrine + the one shared contract
  this slice implements (and resolves its cursor-encoding open question).
- [`series-paging-scope.md`](series-paging-scope.md) — **slice B**, the first consumer: `series.read` grows
  `{limit, cursor, direction, mode:"rows"}` over this keyset.
- [`series-decimation-scope.md`](series-decimation-scope.md) — **slice C**: `mode:"buckets"` rides this
  codec opaquely.
- [`federation-paging-scope.md`](federation-paging-scope.md) — **slice D**: pushes this keyset predicate
  down to external sources.
- [`page-chaining-ui-scope.md`](page-chaining-ui-scope.md) — **slice E**: the callers that chain pages.
- [`../ingest/ingest-scope.md`](../ingest/ingest-scope.md) — the `series` plane + `series.read`
  ([`ingest/read.rs`](../../../rust/crates/host/src/ingest/read.rs),
  [`lb-ingest read.rs`](../../../rust/crates/ingest/src/read.rs)) this primitive serves.
- README `§3` (rules 2 one-datastore / 3 state-vs-motion / 5 capability-first / 6 workspace-wall), `§6.1`
  (API shape).
