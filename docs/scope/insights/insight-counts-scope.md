# Insights scope — the tally rides `insight.list`

## The measurement

The shipped Insights board is five panels over the same `kind:alert` rows — four counter cards and a
roster. Measured on the real ESR data:

| | |
|---|---|
| API calls | 5 (10 in dev — StrictMode doubles every effect) |
| transferred | **1,026,953 bytes** |
| drawn from it | four integers and one list |

Every card fetched 200 records to display one number, and all five read the same rows. The same board
took 0.064 s against a local copy of that data and about 2.4 s against the server, which says the cost
was **network, not database** — a megabyte, not a slow query.

## Owning repos (cross-repo — WORKFLOW-LB §2)

| repo | change |
|---|---|
| **lb** (this) | the tally on `insight.list`, and one database query per call |
| **rubix-ai** | the board that makes one call — see `dashboards/shared-queries-scope.md` |

## Goals

1. **A caller can have the tally without the rows.** `insight.list { limit: 0, counts: true }` returns
   `{total, open, acked, resolved}` and reads no records.
2. **A caller fetching rows gets the tally free.** `counts: true` with a real limit tallies the rows it
   already matched, in memory. Asking the database twice for one answer is the N+1 in disguise.
3. **ONE database query per call, on every path.** No paging loop, no second query, no exceptions.
4. **A tile and its roster cannot disagree.** Both resolve the filter through one builder.

## Non-goals

- **Indexes.** Measured at 197 rows with 39–64% selectivity, a full scan is the right plan and an
  index is maintenance cost for a worse one. Revisit when the row count justifies it, not before.
- **A count-only verb.** `limit: 0` already expresses it. A second verb would be a second capability
  to grant, a second filter path to keep in step, and one more thing to get wrong.
- **Client-side tallying.** That is the shape being retired.

## Intent / approach

### The reply

```jsonc
// insight.list { tags: {kind: "alert"}, limit: 200, counts: true }
{ "items": [ … ],
  "counts": { "total": 197, "open": 78, "acked": 12, "resolved": 107 } }
```

`counts` is ABSENT unless asked for, so every existing caller round-trips byte-identically.

### The three paths, all ONE query

| the call | what runs |
|---|---|
| `limit: 0` | SQL `GROUP BY status`. No rows are read. |
| `counts` absent | the whole filter pushed into SQL, keyset-paged, `LIMIT n+1` as the has-more probe |
| `counts: true` with rows | one SELECT of the matching set, then the tally and the page in memory |

The third case is the one worth stating. The rows and the tally answer the same question, so the
matching set is read once and both come out of it. An earlier version queried for the page and then
queried again to aggregate — one call, two scans, which is exactly the waste this scope exists to
remove.

### The one deliberate asymmetry

`status` is ignored when computing the tally, because the tally IS the per-status breakdown; honouring
a status constraint would make three of the four numbers zero by construction. The page is still
narrowed by it. One predicate builder (`filter_sql`) serves both, with `include_status` as its only
switch — so a tile reading "78 open" and the roster beside it cannot disagree.

## How it fits the core

- **Rule 10.** No special case anywhere: `counts` is a field on the existing query, and the filter
  axes are the ones `list` already had.
- **The workspace wall.** Unchanged — the scan is ws-scoped, and `counts` discloses strictly less
  than the page it summarizes. No new capability: `mcp:insight.list:call` already covers it.
- **State vs motion.** Reads only.

## Testing plan

**The tally.** It matches what `list` returns for each status; an empty table counts zero rather than
erroring; an empty tag allowlist counts nothing, not everything; the severity floor keeps `list`'s
`at_least` meaning; `unassigned` counts only rows with no owner.

**The two paths against each other.** `list` with counts returns the same four numbers as the SQL
path — the test that stops them drifting.

**The absence.** `counts` is absent unless asked for (the byte-clean round-trip every existing caller
depends on).

**`limit: 0`.** Returns the tally and no rows.

**A status-filtered page still reports all four numbers** — the asymmetry above, pinned.

## Risks & hard problems

**`MAX_ROWS` truncation.** A workspace past the cap has a tally over what was read, not over what
exists, and nothing in the reply says so. Today's data is two orders of magnitude below it. If a real
board approaches it, the honest fix is a stated `truncated` flag, not a bigger number.

## Related

- `dashboards/shared-queries-scope.md` — the board that turns five calls into one.
- `caching/dashboard-query-acceleration-scope.md` — `viz.query_batch`, the transport-level sibling.
