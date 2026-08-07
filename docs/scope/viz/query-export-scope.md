# Viz scope — the export row budget: report truncation, raise the ceiling on request

Status: **scope (the ask)**. Backend-only. Promotes to `doc-site/content/public/datasources/` (beside the
decimation + row-cap contracts it completes) once shipped.

Every read verb that feeds a panel silently stops at **10 000 rows** — `MAX_ROWS_PER_FRAME` in
`host/src/viz/query.rs`, and `ROW_CAP` in `crates/federation/src/validate.rs`. For *rendering* that cap is
correct: nobody plots 200 000 points, and the decimation path exists to make that true. For **exporting**
it is wrong twice over: the caller gets a shortened dataset, and — worse — **has no way to know it was
shortened**. Today a client can only infer truncation by comparing `rows.len()` to a magic `10_000` it
should never have been taught. This scope fixes the honesty first and the ceiling second: every capped read
**reports that it capped**, and a caller that explicitly asks for more may have more, up to a node-configured
budget.

**Consumer:** `rubix-ai: docs/scope/frontend/dashboard/data-export-scope.md` (data export as CSV/JSON). That
scope ships **Phase 1 inside today's cap**, surfacing the truncation warning it can only render honestly once
goal 1 lands. Goal 2 is what lets it ship without the warning.

> Read with: `datasources/page-chaining-scope.md` and `datasources/federation-paging-scope.md` (the
> **durable** answer — keyset paging; this scope is explicitly the interim and must not fork a second
> pager), `viz/panel-resolution-scope.md` (decimation — the *render* answer to the same volume problem),
> `datasources/datasources-scope.md` (federate-vs-mirror; `federation` is a native Tier-2 extension, not core).

---

## Goals

1. **Truncation is reported, never inferred.** `viz.query` returns, per frame, `truncated: bool` and the
   `row_limit` that applied; `federation.query` returns the same two fields on its result. Purely additive
   serde fields, no cap change, no behaviour change for an existing caller. **This alone unblocks the
   consumer's Phase 1** and is worth shipping on its own.
2. **An explicit, bounded export budget.** Both verbs accept an optional `max_rows`. Absent → today's
   10 000, byte-identical behaviour. Present → clamped to a node-config ceiling. The caller asks; the node
   decides what it can afford.
3. **The ceiling is configuration, not a constant.** `BootConfig` gains `viz_export_max_rows` (default
   **250 000**). Read from env **only at the binary boundary** (`from_env`), never below the seam. Same field
   on every node — role is config, never a code branch (rule: symmetric nodes).
4. **The same capability wall.** No new capability. `mcp:viz.query:call` / `mcp:federation.query:call` gate
   the larger read exactly as they gate the small one. **A bigger budget is not a bigger authority** — if a
   caller may read the rows at all, it may read more of them; if it may not, the budget never applies.
5. **One contract, applied by each verb's own owner.** The `{max_rows in, truncated + row_limit out}` shape
   is a written convention both verbs implement — core implements it for `viz.query`, the `federation`
   extension implements it for `federation.query`. Core does **not** reach into the extension to do it and
   does **not** learn the name `federation` anywhere new (rule 10).

## Non-goals

- **A `viz.export` verb.** Rejected below.
- **Paging.** `page-chaining` owns that and this scope must not pre-empt it. `max_rows` is a bigger single
  answer, not a cursor.
- **Streaming a file from the node.** The consumer serializes in the browser; the node returns rows as it
  always has.
- **Changing the render path's defaults.** A dashboard panel keeps asking for ≤10 000 rows and keeps getting
  decimated frames. Nothing about a normal board render changes.
- **Raising the federation source-side cap unconditionally.** `ROW_CAP` is pushed down as a real remote
  `LIMIT`; it stays the default precisely so a fat-fingered `SELECT *` cannot pull a warehouse table.

---

## Intent / approach

**Additive on both ends of the existing call, and nothing else.**

```
in    viz.query { panel, scope, max_rows?: u32 }          ← absent = 10_000, unchanged
      federation.query { source, sql, max_rows?: u32 }

out   { frames: [ { …, truncated: bool, row_limit: u32 } ], rows: [...] }
      { columns, rows, truncated: bool, row_limit: u32 }

clamp effective = min(max_rows.unwrap_or(DEFAULT_ROWS), boot.viz_export_max_rows)
```

The clamp is one function in one place per verb, evaluated **before** any allocation sized by it, so a
caller asking for 10 000 000 rows costs a comparison, not a heap. `truncated` is set where the truncation
already happens (`query.rs:308/445/474`, `federation` `df.limit`/`cap_direct_sql`) — the two sites that
today drop rows on the floor without a word.

For the federation path the cap is pushed down as a remote `LIMIT`, so a larger `max_rows` changes what the
*source* returns, not what we discard locally — the pushdown discipline `federation-pushdown-scope.md`
established is preserved, not bypassed.

### Rejected: a dedicated `viz.export` verb

It reads well and is wrong. It would need to re-resolve panels, sources, variables, and the transform
pipeline — i.e. duplicate `viz.query`'s entire resolution path — and then that duplicate has to be kept
honest forever. It would also need its own capability, which invites the exact confusion goal 4 refuses: a
"you may export" grant distinct from "you may read" is a second, weaker authority model over the same rows.
One verb, one resolution, one cap wall; the request just carries a budget.

### Rejected: raise `MAX_ROWS_PER_FRAME` to something big

Every board render then pays for it, the decimation work is undermined, and one heavy panel can pin a node's
memory. The cap is right for the default path; the *request* is what should vary.

### Why `truncated` ships first and separately

Goal 1 is a few serde fields and two boolean assignments, needs no config, cannot regress a caller, and
turns the consumer's export from *quietly wrong* into *honestly limited*. Goal 2 is the one that touches
`BootConfig` and memory characteristics. Ship 1, tag it, let the consumer render the warning; ship 2 when
the consumer's telemetry says the cap actually bites.

## How it fits

- **Capabilities & the deny path.** No new cap name. A caller lacking `mcp:viz.query:call` is denied before
  `max_rows` is ever parsed — the budget is not a side door. Test: a denied caller with a huge `max_rows`
  gets the same denial as one with none, and no allocation happens.
- **Workspace isolation.** Untouched: the resolution runs under the caller's workspace exactly as today. A
  larger budget returns more of *the caller's own* rows.
- **Symmetric nodes.** `viz_export_max_rows` is a `BootConfig` field present on every node. A small embedded
  node sets it low; a cloud node sets it high. No `if cloud`.
- **Env is a binary concern.** `LB_VIZ_EXPORT_MAX_ROWS` is read in the binary's `from_env` and written into
  `BootConfig`. Nothing under the seam reads env.
- **Extensions (rule 10).** `federation` implements the convention in its own crate. Core gains no branch on
  an extension id and no knowledge that `federation.query` exists beyond the generic tool resolution it
  already does. Any other read verb may adopt the same two fields later with no core change.
- **Data / motion / store.** No new table, no store schema change, no bus traffic, no outbox item. Reads only.
- **Memory.** Bounded by `viz_export_max_rows` × frame width. The ceiling is the contract: a node operator
  who sets it to 5 000 000 has chosen that, and the clamp makes it the only number that matters.
- **lb-viz.** Transforms are pure and linear in row count; a larger frame set costs proportionally more CPU
  and nothing structurally. Worth one benchmark, not a redesign.

## Example flow

1. rubix-ai's export dialog asks for the whole board. Each panel's export resolution calls
   `viz.query { panel, scope }` — **no `max_rows`** — because the user did not ask for more than the default.
2. One frame comes back `{ truncated: true, row_limit: 10000 }`. The dialog renders *"Zone temps hit the
   server's 10,000-row cap — narrow the time range"*, and the file's notes carry the same fact. **(Goal 1
   alone, shipped.)**
3. Test picks *Export full dataset*. The dialog re-resolves that panel with `max_rows: 250000`.
4. The node clamps `min(250_000, boot.viz_export_max_rows = 250_000)`, resolves the sources with the larger
   budget, runs the transform pipeline, and returns 187 412 rows with `truncated: false, row_limit: 250000`.
5. The dialog drops the warning and writes the file. Nothing else on the board changed: the *rendered*
   panel still holds its decimated 10 000.
6. A second workspace's viewer, lacking `mcp:viz.query:call`, sends the same request and gets the same
   denial it has always got.

## Testing plan

Real store (`mem://`), real bus, real gateway through the lib API — no mocks (rule 9).

**Mandatory categories:** capability-deny ✅ · workspace-isolation ✅ · offline/sync N/A (read-only, no
durable state) · hot-reload N/A (no ABI/manifest surface).

- **Unit — clamp.** `max_rows` absent → 10 000. Below the ceiling → honoured. Above → the ceiling. Zero and
  `u32::MAX` → the ceiling, no panic, no allocation sized by the request.
- **Unit — the flag.** A frame at exactly the limit with more rows available → `truncated: true`. A frame at
  exactly the limit that *was* the whole result → **`truncated: false`** (this is the subtle one: it needs
  the `limit + 1` probe, the same trick `page-cursor-scope.md` uses, not a length comparison — a
  length-comparison implementation reports a false positive on every exact-fit result and is the bug this
  test exists to catch).
- **Integration — `viz.query`.** Seed a series with 25 000 points. Default request → 10 000 rows,
  `truncated: true`. `max_rows: 30000` → 25 000 rows, `truncated: false`. Assert exact counts and that the
  transform pipeline ran over the full set.
- **Integration — `federation.query`.** Against the sqlite demo source: assert the larger `max_rows` is
  pushed down as a remote `LIMIT` (structural assertion on the plan, as `row_cap_clamps_via_remote_limit`
  already does) — not fetched-then-discarded.
- **Capability-deny.** No `mcp:viz.query:call` + `max_rows: 250000` → denied identically to the no-budget
  call; same for `federation.query`.
- **Workspace isolation.** A ws-B token with a large budget over a ws-A panel resolves nothing of ws-A.
- **Config.** `viz_export_max_rows` set low on `BootConfig` clamps a large request to it; the default is
  250 000; `LB_VIZ_EXPORT_MAX_ROWS` is honoured only through the binary's `from_env`, and a below-seam env
  read would fail the existing boundary test.
- **Benchmark (not a gate).** Transform pipeline wall-clock at 10 k vs 250 k rows, recorded so the ceiling's
  default is an informed number rather than a guess.

## Risks & hard problems

- **A big budget becomes the default by habit.** If the consumer starts sending `max_rows` on every render,
  the render path's memory profile silently changes. Mitigation: the consumer sends it **only** on an
  explicit export action (stated in its scope), and the flag/telemetry makes over-use visible.
- **Exact-fit false positives.** See the unit test above — the `limit + 1` probe is the implementation, and
  a naive `len() == limit` is the bug.
- **This becomes a de-facto pager.** `max_rows` must never grow an `offset` sibling; that would be a second,
  worse pager competing with `page-chaining`. If a caller needs to walk, the answer is the cursor contract,
  and this scope should be pointed at as the thing that must *not* be extended.
- **Federation sources that ignore pushdown.** A source that cannot push the `LIMIT` down fetches more than
  we want before we clamp. That is the same condition `federation-paging-scope.md` handles by routing to the
  mirror; here it means a large `max_rows` on a non-pushdown source should be **refused with a reason**
  rather than served slowly.

## Open questions

1. **Is 250 000 the right default ceiling?** It is a guess pending the benchmark. A node on constrained
   hardware may want 50 000. Answerable from the benchmark row of the testing plan.
2. **Should `truncated` also carry `available` (the true count)?** It would make the warning quantitative
   ("10 000 of ~187 000"), but a true count means a `COUNT(*)` — exactly the cost `page-cursor-scope.md`
   refuses. Recommendation: **no**; the boolean plus the applied limit is honest and free.
3. **Should a non-pushdown federation source refuse a large `max_rows` or serve it slowly?** Recommendation:
   **refuse with a typed error naming the mirror path**, matching the paging scope's stance.
4. **Do the other read verbs (`series.read`, `query.run`, `store.query`) adopt the convention now or on
   demand?** Recommendation: **write the convention down here, adopt on demand** — `viz.query` and
   `federation.query` are what the consumer actually calls.

## Related

- **Downstream consumer:** `rubix-ai: docs/scope/frontend/dashboard/data-export-scope.md` — the export UX
  this unblocks. It ships Phase 1 without goal 2; it renders an honest warning only with goal 1.
- `datasources/page-chaining-scope.md`, `datasources/page-cursor-scope.md`,
  `datasources/federation-paging-scope.md` — the durable answer this is the interim for.
- `viz/panel-resolution-scope.md` — decimation, the *render*-side answer to the same volume problem.
- `datasources/federation-pushdown-scope.md` — why the federation cap is a remote `LIMIT` and must stay one.
- `viz/grafana-parity-backend-scope.md` — the sibling backend scope whose downstream-UI split this mirrors.
- Code: `rust/crates/host/src/viz/query.rs` (`MAX_ROWS_PER_FRAME`, the three truncate sites),
  `rust/crates/federation/src/validate.rs` (`ROW_CAP`, `cap_direct_sql`),
  `rust/crates/federation/src/query.rs` (`df.limit`).
