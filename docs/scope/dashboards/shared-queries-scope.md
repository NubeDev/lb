# Dashboards scope — named queries on the board, `from` + `pick` on the panel

A board's panels each fetch for themselves. When several want the same data, the board pays for it
several times. Measured on the shipped ESR Insights board — four counter cards and a roster, all over
`kind:alert`:

| | |
|---|---|
| API calls | 5 (10 in dev — StrictMode doubles every effect) |
| transferred | **1,026,953 bytes** |
| database reads | 5 |
| drawn from it | four integers and one list |

Every card fetched 200 records to display one number, and all five read the same rows.

An earlier attempt inferred the sharing: panels whose filters matched were answered from one read.
That is a **guess**, and it fails the moment two panels legitimately differ — a roster filtered to
`severity: critical` beside counters that count every severity wants ONE call and gets two. It is
also invisible: nothing on the board says these panels share, and editing one panel's filter silently
splits the read with no error and no sign.

So the sharing becomes something an author **states**, not something the runtime deduces.

## Owning repos (cross-repo — WORKFLOW-LB §2)

| repo | change |
|---|---|
| **lb** | `queries` on the `Dashboard` record + the `dashboard.save` whitelist |
| **rubix-ai** | the `pick` evaluator, the board runtime that fires each query once, the editor |

The record change is lb's because the struct **drops unknown top-level keys** — a `queries` block
that is not a typed field is silently discarded on the first save, which is the worst possible
failure: the author configures it, the board works until reload, and then it is gone.

## Goals

1. **A board names its queries once.** One place to read, one place to edit.
2. **A panel says where its data comes from and which part it wants** — `from` + `pick`.
3. **Each named query fires ONCE per board load**, however many panels read it.
4. **`pick` selects rows as well as fields** — a path (`counts.acked`) or a filtered set
   (`items where status = open`).
5. **No code change to add a panel.** A new counter is configuration.
6. **Tool-agnostic.** `insight.list` is the first user; a board may name a `viz.query` the same way.

## Non-goals

- **A query language.** `pick` is a path plus equality filters. Anything needing joins or expressions
  is a datasource's job, not a dashboard field.
- **Cross-board sharing.** A named query belongs to one board. Sharing across boards is a datasource.
- **Replacing per-panel queries.** A panel with no `from` keeps fetching for itself, exactly as today.
  This is additive: every existing board behaves identically.
- **A cache.** Each query runs once per board load, not once per session. A triage plane is live.

## Intent / approach

### The record

```jsonc
// dashboard.queries — named, board-level
{ "alerts": { "tool": "insight.list",
              "args": { "tags": {"kind":"alert"}, "limit": 200, "counts": true } } }
```

### The panel

```jsonc
{ "title": "Total",        "from": "alerts", "pick": { "path": "counts.total" } }
{ "title": "Open",         "from": "alerts", "pick": { "path": "counts.open" } }
{ "title": "Acknowledged", "from": "alerts", "pick": { "path": "counts.acked" } }
{ "title": "Resolved",     "from": "alerts", "pick": { "path": "counts.resolved" } }
{ "title": "Critical now", "from": "alerts",
  "pick": { "path": "items", "where": { "severity": "critical" } } }
{ "title": "Roster",       "from": "alerts", "pick": { "path": "items" } }
```

Six panels, **one call**. The last two differ by severity — the case the inferred design could not
share — and here it is one query with two picks.

### `pick`

```jsonc
{ "path": "items", "where": { "status": "open" }, "limit": 50 }
```

- `path` — dotted, into the reply. Absent ⇒ the whole reply.
- `where` — equality on each named field, AND-composed. Rows only.
- `limit` — after `where`.

Equality only, deliberately. The moment `pick` grows operators it is a query language living in a
dashboard field, and the right answer becomes a second named query.

### Runtime

The board resolves `queries` once per load and holds the results; a panel with `from` reads its slice
and **never fetches**. A panel whose `from` names a missing query renders an error naming it — not an
empty panel, which reads as "no data" and sends the author looking in the wrong place.

## How it fits the core

- **Rule 10.** The runtime never inspects `tool`; it calls what the board names and applies `pick` to
  whatever comes back. Insights get no special case — the special-casing this REPLACES is exactly the
  `select: "rows" | "count"` field the inferred design needed.
- **The workspace wall.** A named query is called as the viewer, through the same gate as a panel's
  own call. Naming a query on a board grants nothing.
- **State vs motion.** Reads only.

## Testing plan

**The evaluator (pure).** Path into nested objects; a missing path; `where` over rows; `where` with a
key the rows lack (must select nothing, never everything); `limit` after `where`; a non-array `path`
with a `where` (a stated error, not a silent empty).

**The runtime.** N panels naming one query ⇒ ONE call, asserted over a recording transport, including
under StrictMode's double mount. A missing `from` renders a named error. A panel with no `from` still
fetches for itself — the additive property every existing board depends on.

**The record.** `queries` survives `dashboard.save` → `dashboard.get` (it is the drop-unknown-keys
trap that makes this the one test that must exist).

## Risks & hard problems

**`pick` is a stringly-typed contract.** `counts.acked` is a path into a reply shape the dashboard
does not own. If the verb's shape changes, the panel breaks at render with no compile-time signal.
Mitigation: a missing path is a stated error naming the path, never a blank panel.

**One failure, many panels.** A named query that fails takes out every panel reading it. That is
correct — they have no data — but the message must name the QUERY, or six panels show six mysteries.

**An author can point `from` at a query whose shape does not fit the panel.** Nothing prevents a
counter picking `items`. It renders wrong rather than failing. Accepted: the same is true of any
mis-bound panel today.

## Open questions

1. **Should a panel be able to override its query's args?** (a counter wanting a different range).
   Proposed: NO for now — that is a second named query, and overrides are how one call quietly
   becomes six again.
2. **Refresh.** Does the board's refresh tick re-run every named query, or only those with a reader
   on screen? Proposed: every one — a stale number is worse than a wasted read.

## Related

- `insights/insight-batch-scope.md` — the counts + batch work this builds on; its `select` field is
  what this replaces.
- `caching/dashboard-query-acceleration-scope.md` — `viz.query_batch`, the transport-level sibling.
