# Insights scope — tag echo: the record carries its own facets

Status: **shipped** (2026-07-30, issue #119 slice 1 — the field + the raise-path write; the backfill
job is still open, see §Backfill and "Open questions after building" below). Session:
[`sessions/insights/insight-tag-echo-session.md`](../../sessions/insights/insight-tag-echo-session.md).
Public: [`doc-site/content/public/insights/insights.md`](../../../doc-site/content/public/insights/insights.md).

Tags are the insight's dimension plane — building, asset type, data type, priority, category,
classification — persisted to the shipped tag graph on every raise and used to *filter*
`insight.list`. But they are **not on the record**. A caller can ask "give me every open
insight in Chullora" and get rows back; it cannot then *display* which building each row is
in without a second round trip to `tags.find` per insight. So the one thing tags are most
wanted for — a roster with dimension columns, the actual shape operators asked for — is
exactly the thing they can't do. This scope echoes the resolved facets onto the record as a
read-only projection, on both `get` and `list`.

Logged as a "sibling gap" in `insight-evidence-scope.md` §Related; promoted here because it
blocks the roster that [`insight-analysis-scope.md`](insight-analysis-scope.md) and
[`insight-triage-scope.md`](insight-triage-scope.md) otherwise complete.

## Goals

- A `tags: BTreeMap<String, String>` on the `Insight` record, echoed by **both**
  `insight.get` and `insight.list`, so a roster renders dimension columns from the list
  response alone — no N+1 `tags.find`.
- **The tag graph stays the source of truth.** The echo is a denormalized projection,
  written only by the raise path that already owns tag application, never edited directly.
- **Cheap.** No new read on the hot path: the raise path *already* materializes the full
  facet set for subscription matching and throws it away. The echo is that value, persisted.
- **Additive and safe.** Absent (empty) on every existing record; a reader that ignores it is
  unaffected; no verb changes shape for an old client.
- **Self-healing.** A record whose echo is stale or empty converges on the next raise, and
  the tag graph can always rebuild it.

## Non-goals

- **Not a second write path.** No `insight.tag` verb, no tag editing through the insight. Tags
  are applied at raise (`Source::Producer` provenance) and via the existing `tags.*` verbs.
  Adding a mutation door here would create two writers for one truth. A human wanting to
  reclassify a finding comments (triage scope) or the producer re-raises.
- **Not the filter path.** `insight.list { tags }` keeps resolving through `lb_tags::find`
  against the graph — the echo is for *display*, not for querying. See "Intent" for why
  filtering off the echo is deliberately rejected.
- **Not high-cardinality data.** The echo inherits the tag plane's cardinality rule
  unchanged: dimensions only. Per-asset identity (`WM-CHU-01`) stays in `dedup_key`.
- **Not provenance.** The graph holds who applied each tag and when; the echo is flat
  `{k: v}`. A caller needing provenance reads `tags.of`.
- **No new capability.**

## Intent / approach

**A denormalized read-only projection on the record, written by the raise path from the value
it already computes.**

The host's `insight_raise` currently does this (`host/src/insight/raise.rs`): applies each tag
to the graph, then — only when subscriptions exist — calls `materialize_facets` to read the
insight's *full* facet set back out of the graph (covering tags from prior raises of the same
`dedup_key`, not just this firing's) so the subscription matcher can test facet filters. That
materialized map is exactly the echo, and it is discarded today. This scope persists it.

Two consequences worth stating plainly, because they're the whole design:

- **The echo must be materialized on every raise, not just when subs exist.** Today the read
  is conditional (`if !subs.is_empty()`). The echo makes it unconditional — one `tags.of` per
  raise on the hot path. That is the real cost of this scope, and it's accepted: it's a single
  indexed graph read next to a record write plus an occurrence append, and it removes an N+1
  from every roster load. If it ever bites, the fix is to merge this raise's declared tags
  into the *stored* echo instead of re-reading (correct except when tags were changed
  out-of-band via `tags.*`), which is a strictly local optimization.
- **The echo is written from the graph, never from `RaiseInput.tags` alone.** A raise declares
  only the tags it knows; the record must carry the union across all raises of that key, or a
  rule that stops sending `classification` would blank a column. Materializing from the graph
  gets the union for free and self-heals a stale echo on the next firing.

**Why echo rather than "just query the graph in the UI".** The alternative is a client-side
`tags.find`/`tags.of` fan-out per page. That's N+1 on the roster's hot path, needs a second
capability (`mcp:tags.of:call`) that a read-only insight viewer otherwise doesn't need — so it
would *widen* the caps a roster requires — and it puts join logic in every consumer. The echo
keeps the roster one call under one cap.

**Why filtering still goes through the graph, not the echo.** Once tags are on the record, a
`WHERE tags.building = 'x'` scan looks tempting and would let us delete the `find` path. It's
rejected: the graph is the source of truth, and a filter that reads a *projection* silently
returns wrong results whenever the projection is stale (a tag applied out-of-band via `tags.*`
without a subsequent raise). Filtering must be correct even when display is briefly behind, so
the two paths stay split — graph for truth, echo for display. This also keeps the facet
intersection semantics (`tags.find`) as the single definition of what a facet match means.

**Rejected: a separate `insight_tags` table.** Same argument the evidence scope settled — a
join for a handful of short strings read only alongside their parent. The graph already *is*
the normalized store; a third copy is worse than a projection on the record.

**Rejected: echoing onto the occurrence ring.** Tags are a property of the insight, not of one
firing, and the ring evicts.

**Dedup: the echo refreshes on every raise** — like `evidence`, unlike `title`/`body`. It's a
projection of current truth; a stale dimension column is a wrong column. Note the contrast
with [`insight-triage-scope.md`](insight-triage-scope.md), where human facts are untouched by
raise entirely. Three different dedup behaviours now coexist on this record, which is why each
scope states its own explicitly:

| Field class | On re-raise | Why |
|---|---|---|
| `title`, `body` | first-raise-wins | producer prose, historically stable (its own open question) |
| `evidence`, `analysis`, `tags` | refresh on supply / recompute | bindings + projections of *current* truth |
| `assigned_to`, comments | **untouched** | human facts a machine must never overwrite |

## How it fits the core

- **Tenancy / isolation:** unchanged — a field on the existing `insight:{ws}:{id}` record, and
  `materialize_facets` already reads the graph inside the workspace namespace. ws-B cannot see
  ws-A's facets through the echo any more than through the graph. Re-pinned by the mandatory
  isolation test below, because a denormalized copy is exactly where a leak would hide.
- **Capabilities:** **no new capability.** Written by the already-gated
  `mcp:insight.raise:call`; read by the already-gated `mcp:insight.get|list:call`. This is a
  net *narrowing* of what a roster needs: today a UI wanting dimension columns needs
  `mcp:tags.of:call` on top of the insight read caps; after this it needs only the insight read
  cap. Worth stating in the deny test — the echo must be readable by a token holding
  `insight.list` and **no** tags caps at all.
- **Placement:** either. No reactor, no motion, no election.
- **MCP surface (API shape, §6.1):**
  - **CRUD:** no new verb, no new field on `RaiseInput` (`tags` already exists). The echo is
    host-computed, never caller-supplied — a caller-supplied `tags` on the *record* is ignored,
    like `producer`.
  - **Get / list:** no new verb. `insight.get` and — the point of the scope —
    **`insight.list` both echo `tags`**. This is the one place the get-vs-list boundary is
    drawn *differently* from `evidence`/`analysis`, and deliberately: those are large,
    detail-only payloads (SQL, prose paragraphs); tags are a handful of short low-cardinality
    strings that exist **to be columns**. Excluding them from `list` would defeat the entire
    purpose. The boundary rule is "does the roster render it", not "is it on the record".
  - **Live feed:** the SSE `RaiseEvent` does **not** carry tags (it carries ids + status; a
    roster re-reads). Same disposition as `evidence`/`analysis`.
  - **Batch:** N/A. But see "backfill" below — the one-off migration for existing records is a
    **job**, not a blocking call.
- **Data (SurrealDB):** one new map field on the existing `insight` table. No new table, no new
  index. Size-guarded by the tag plane's own cardinality rule plus a cap on the echo (below).
- **Bus (Zenoh):** nothing new.
- **Sync / authority:** ordinary workspace data. The graph is authoritative; the echo is
  derived and idempotently recomputed, so it needs no merge semantics.
- **Secrets:** none.
- **SDK/WIT impact:** **none** — additive optional field, unchanged host-callback path.
- **Skill doc:** extend `skills/insights/SKILL.md` — the raise/list walkthrough must show tags
  coming back on `list` and state that the graph is the write path (the echo is read-only).

### Backfill

Existing records have an empty echo until their next raise. For a resolved insight that never
fires again, that's permanent — so the roster would show blank dimension columns on exactly the
historical rows an operator is reviewing. The backfill is a **job** (never a blocking call —
it's an unbounded table walk): for each insight in the workspace, `tags.of` → write the echo.
Idempotent, resumable, and safe to re-run; it's the same shape as the `heal_ts` precedent
already in `host/src/insight/heal_ts.rs`. Sequencing: ship the field + raise-path write first,
then the backfill job, so a partial backfill is never distinguishable from "not yet re-raised".

## Example flow

1. A rule raises the Chullora finding with `tags: { building: "chullora-dc",
   asset_type: "water-meter", data_type: "water", priority: "medium",
   classification: "plumbing" }`.
2. The host writes the record, applies each tag to the graph with `Source::Producer`
   provenance, then materializes the insight's full facet set (now unconditionally, not only
   when subscriptions exist) and stores it as the record's `tags` echo.
3. A month later the site is re-classified. An admin applies
   `classification: "mechanical"` through the existing `tags.*` verb — the graph is updated,
   the echo is briefly stale.
4. The rule fires again that night. The echo is recomputed from the graph and now reads
   `mechanical`. Self-healed, no special path.
5. An operator loads the roster: **one** `insight.list` call returns rows carrying title,
   severity, status, `last_ts`, `assigned_to`, and the tag map — every dimension column renders
   with no follow-up request, under one capability.
6. They filter to `{ tags: { building: "chullora-dc" }, status: "open" }`. That resolves
   through `lb_tags::find` against the **graph** (not the echo), so the result is correct even
   for a record whose echo hasn't caught up to step 3 yet.

## Testing plan

Per `scope/testing/testing-scope.md`, against the **real** store (`mem://`), the real tag
graph, and a real spawned gateway — no mocks (rule 9). Mandatory categories:

- **Capability deny (mandatory).** A token with `mcp:insight.list:call` and **no** `tags.*`
  caps still gets the echo (the narrowing this scope buys); a token without
  `mcp:insight.list:call` gets an opaque 403 and no facet data in the error. Per the deny-test
  lesson: a real id and a fictional id must produce **identical** errors, and **revert-check**
  the gate rather than trusting an inner layer.
- **Workspace isolation (mandatory).** ws-B's `get`/`list` never returns ws-A facets — assert
  on a ws-A insight and a ws-B insight sharing the *same tag key and value*, so a leak through
  the shared tag vocabulary would show up. This is the test that matters most: a denormalized
  copy of graph data is precisely where cross-workspace bleed hides.
- **Offline / sync, hot-reload:** N/A.

Key cases:

1. **Echo lands on raise and appears in `list`.** Raise with three tags → `list` rows carry
   all three without any `tags.find`. Assert on the **list** response specifically; a test that
   only checks `get` would pass while the roster stayed broken.
2. **Union across raises, not last-write.** Raise with `{building}`, re-raise the same
   `dedup_key` with `{asset_type}` only → the echo carries **both**. Revert-check by writing
   the echo from `RaiseInput.tags` instead of the graph and confirming red — this is the bug
   the scope exists to avoid and the one a future refactor will reintroduce.
3. **Self-heal after out-of-band change.** Apply a tag via `tags.*` → echo is stale → re-raise
   → echo matches the graph.
4. **Filtering still reads the graph.** Apply a tag out-of-band, do **not** re-raise, then
   filter `list` by it: the record **matches** (graph is truth) even though its echo is stale.
   This pins the split — if someone "simplifies" the filter to read the echo, this goes red.
5. **Echo is not caller-writable.** A raise body supplying a record-level `tags` projection
   distinct from its declared tag map is ignored (the `producer` host-stamp precedent).
6. **Unconditional materialization.** With **zero** subscriptions in the workspace, the echo is
   still written — the case the current conditional `if !subs.is_empty()` would skip. Revert-
   check: restore the condition and confirm red.
7. **Backfill job.** Seed pre-field records (real store, pre-field JSON shape) → run the job →
   echoes match `tags.of`; re-run → idempotent; a record with no tags gets an empty map, not an
   error.
8. **Cardinality guard.** An insight carrying an absurd number of tags doesn't produce an
   unbounded record — the echo cap rejects/truncates-loudly per the size-guard contract
   (`validate_evidence_size` precedent: never a silent truncation).
9. **Live verification in the product** (not just the suite — `cargo test` has historically not
   caught the real bugs here): load the roster against the running node, confirm dimension
   columns render from a single request (check the network panel for the absent N+1) and that
   filtering still works.

## Risks & hard problems

- **Two copies of one truth, and the copy is the one people read.** Every denormalization risks
  divergence; here the divergence window is "until the next raise", which for a resolved
  insight is *forever*. Mitigations: the backfill job, self-heal on raise, and the deliberate
  rule that filtering never trusts the echo. The residual risk is a UI that shows a stale
  dimension on a long-resolved finding — acceptable, but it must not be presented as
  authoritative in a report.
- **One extra graph read on the raise hot path.** Making `materialize_facets` unconditional
  costs a `tags.of` on every raise, including the flapping-sensor case that raises every 15
  minutes across thousands of keys. Named above with its local optimization; worth measuring
  in the implementing session rather than assuming, per the drain-backpressure lesson (the
  blamed cause there was disproven by measurement).
- **The cardinality cap is now load-bearing in a second place.** A workspace that blows the 10k
  tag-node cap already degrades tag writes; now it also produces fat insight records. The echo
  cap is belt-and-braces, but the real defence is still producer discipline — documented in the
  skill doc, unenforceable by the platform.
- **`list` now carries data whose size a producer controls.** Tags are short and bounded by
  discipline, not by schema. A producer applying twenty facets makes every roster page heavier
  for everyone. The echo cap bounds the worst case; there's no per-key allowlist and this scope
  doesn't add one.
- **Backfill on a large table.** A workspace with hundreds of thousands of insights makes the
  backfill a long walk. It's a resumable job for that reason, but it competes with live traffic
  and should be run deliberately, not on boot — unlike `heal_ts`, whose boot driver is cheap.

## Resolved decisions

Stated here rather than as open questions, so the implementing session has no ambiguity:

1. **Echo on `list`, not just `get`** — the get-vs-list boundary is "does the roster render
   it", and dimension columns are the entire ask. Contrast `evidence`/`analysis`, correctly
   `get`-only.
2. **Materialize from the graph, not from `RaiseInput.tags`** — the record must carry the union
   across raises. Pinned by test §2.
3. **Filtering keeps reading the graph** — correctness over deleting a code path. Pinned by
   test §4.
4. **No tag-write door on the insight** — one writer for one truth.
5. **Refresh on every raise** — it's a projection of current truth (see the dedup table above).
6. **Backfill is a job, shipped after the field** — never a blocking call, never a boot walk.

## Open questions after building

All six resolved decisions held as written and are pinned by tests (2, 3 and 6 by revert-check).
The implementing session raised three things the scope did not anticipate:

1. **The tag *write* at raise was gated on `mcp:tags.add:call`, not on `insight.raise`** — so an
   ordinary producer's declared tags never reached the graph, silently, and the echo would have been
   built from the matcher's declaration fallback (i.e. exactly the union bug decision 2 forbids).
   Fixed in this slice by applying + reading the graph raw, host-internally, behind the already-passed
   `mcp:insight.raise:call` gate — the `insight_list` precedent. **No capability was widened.** Full
   write-up: [`debugging/insights/producer-tags-never-reached-the-graph.md`](../../debugging/insights/producer-tags-never-reached-the-graph.md).
2. **`tags.*` has no wire door.** `call_tags_tool` is built and gated but has no entry in the
   dispatcher's host-native table, so `{"tool":"tags.add"}` over `/mcp/call` returns `no such tool`
   (verified live). The scope's step-3 story — "an admin re-classifies through the existing `tags.*`
   verb" — therefore has no caller-reachable path today; the in-suite tests exercise the bridge
   directly. Wiring a verb family into the dispatcher (+ a deny test per verb) is its own scope, not
   a tag-echo change. **Open.**
3. **Same-key, multi-source edges are undefined in a flat echo.** Edge identity is
   `(entity, tag, source)`, so a `Producer` `classification=plumbing` and a `Human`
   `classification=mechanical` coexist and the flat map keeps whichever `tags.of` returns last. In
   contract (the echo is explicitly not provenance) but unchosen. Needs a rule before
   [`insight-triage-scope.md`](insight-triage-scope.md) lets humans re-classify. **Open.**

The **backfill job** remains as scoped and sequenced (§Backfill): `set_tags_echo` shipped idempotent,
resumable and map-taking precisely so the job is a table walk plus a call. Not on a boot driver.

## Related

- Parent: [`insights-scope.md`](insights-scope.md) — §"Tags" (the tag plane + the cardinality
  rule this inherits) and §"MCP surface".
- [`insight-evidence-scope.md`](insight-evidence-scope.md) — logged this as the "sibling gap";
  its size-guard and no-new-capability arguments are reused, and its `get`-only boundary is the
  case this scope deliberately diverges from (and says why).
- [`insight-analysis-scope.md`](insight-analysis-scope.md) and
  [`insight-triage-scope.md`](insight-triage-scope.md) — the other two thirds of the
  "more fields on an insight" ask; the roster they describe is **blocked on this scope** for its
  dimension columns.
- [`insight-subscriptions-scope.md`](insight-subscriptions-scope.md) — owns
  `materialize_facets`, the value this scope persists; making it unconditional is the one
  behavioural change to that path.
- `scope/tags/tags-scope.md` — the graph, facet intersection, provenance, and the 10k
  per-workspace tag-node cap (deny on exceed).
- `scope/jobs/jobs-scope.md` — the backfill job; `host/src/insight/heal_ts.rs` is the
  in-repo precedent for a one-off insight-table heal.
- `skills/insights/SKILL.md` — extended by the implementing session.
- README §3 (rules 2, 5–7), §6.5.
