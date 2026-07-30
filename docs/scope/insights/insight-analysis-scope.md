# Insights scope — `analysis`: the finding explains itself

Status: **shipped** (2026-07-30, issue #119 slice 2). Session:
[`sessions/insights/insight-analysis-session.md`](../../sessions/insights/insight-analysis-session.md).
Public: [`doc-site/content/public/insights/insights.md`](../../../doc-site/content/public/insights/insights.md).

The shipped `evidence` field says **where the data is** — datasource, plottable series,
threshold, window. It does not say **what the producer concluded**: why this rule fired, what
the normalised metric was, what it was compared against, how far off it was, and what it's
likely to cost. Today that reasoning has two homes, both wrong: free-form `body` (opaque JSON,
so no consumer can render it consistently and no agent can read it reliably) or the `title`
(one line, so it gets truncated into uselessness). This scope adds an optional **`analysis`**
beside `evidence` — a small, closed set of short prose fields stating the producer's own
explanation of the finding, structured enough to render in a fixed drawer layout and to feed an
agent, narrow enough to stay off the roster.

This is the second half of `insight-evidence-scope.md`: that scope made the finding state its
**data**; this one makes it state its **reasoning**.

## Goals

- An optional **`analysis`** on `insight.raise`, persisted and echoed by `insight.get`: a closed
  struct of short, nullable text fields (trigger logic, root cause hypothesis, normalised
  metric, benchmark context, deviation, estimated impact).
- **Consumers stop parsing prose.** A detail drawer renders a stable labelled layout, and the
  insights-analyst persona reads named fields instead of sniffing `body` shapes.
- **Additive and safe.** Absent on every existing record and every producer that sets none; no
  record fails to decode; a reader that ignores it is unaffected (the `evidence` rollout
  precedent).
- **Generic.** The vocabulary is analytical, not domain-specific: no FDD term, no rule-family
  term, no consumer named anywhere (rule 10). "Deviation" and "benchmark" are as domain-neutral
  as "threshold" already is.
- **Cheap to skip.** A producer that knows only one of the six sets one field.

## Non-goals

- **Not a replacement for `body`.** `body` stays the free-form producer-owned detail — arbitrary
  rows, scores, links. `analysis` is the narrow *named* subset that every consumer can render.
  A producer wanting a seventh dimension uses `body`; that is the escape hatch, and its
  existence is why this struct can stay closed.
- **Not computed by the node.** The host does not derive, verify, or recompute any field. It
  does not check that `deviation` agrees with `evidence.threshold`, and it never runs a query to
  fill one in. These are the *producer's claims*, stored verbatim — the same posture `evidence`
  takes on SQL it never executes.
- **Not the human plane.** Operator prose (comments) and ownership live in
  [`insight-triage-scope.md`](insight-triage-scope.md). `analysis` is the machine's statement;
  a human correcting it comments, and never overwrites it.
- **Not dimensions.** Group / building / asset type / data type / priority / category /
  classification are low-cardinality **facets** and ride the shipped tag graph (umbrella
  §"Tags"); per-asset identity rides `dedup_key`. None of them belong here. **Caveat worth
  knowing before promising a roster:** tags are persisted and drive `insight.list` facet
  filtering but were **not echoed** on the `Insight` record, so a UI could not *display* them
  as columns without a separate `tags.find`. That gap is closed by
  [`insight-tag-echo-scope.md`](insight-tag-echo-scope.md), which is where the dimension
  columns come from — **not** from this scope. Note the deliberate asymmetry: tags ride
  `insight.list` (they exist to be columns), `analysis` does not (it exists to fill a drawer).
- **Not a metrics plane.** `deviation` and `estimated_impact` carry a number *and* the
  producer's prose, but this is not a place to accumulate arbitrary measurements — six named
  fields, closed. Aggregate analytics over findings is the data plane's job.
- **No new capability.** Additive fields on an existing gated verb, exactly as `evidence`
  argued.

## Intent / approach

**A closed struct of six optional fields — four prose, two quantities — beside `evidence`, not
inside it.**

Beside rather than inside because the two have different lifetimes and different readers.
`evidence` is a *binding* a panel resolves at read time (and therefore refreshes on re-raise,
so a renamed table can heal). `analysis` is a *statement about a firing* — closer to `title`
and `body`. Nesting the reasoning inside the data binding would also push `evidence` past its
4 KB descriptor cap for reasons that have nothing to do with SQL, and would force a reader that
wants only the prose to pull the queries too.

**Why a closed struct and not a `Map<String, String>`.** A free map is the obvious flexible
choice and it is wrong here for the reason a closed struct is wrong elsewhere: the value of
these fields is that *every* consumer renders the same six labels in the same order, and an
agent prompt can name them. A map gives every producer a private vocabulary
(`root_cause` vs `rootCause` vs `cause`), which is `body` again with extra steps. The
platform's own lesson cuts the other way too, though, and this scope accepts the cost
knowingly: a closed struct means **a new field is silently dropped until it is added to the
Rust type** — the failure mode that has bitten this repo repeatedly (dashboard `Variable`,
`Prefs`). Mitigation is documentation, not code: the struct is closed *on purpose*, `body` is
the documented overflow, and the skill doc must say so.

**`deviation` / `estimated_impact` are a `Quantity`: an optional number + unit *and* an
optional note.** These are the two fields that want to be sorted — "show me today's findings
by cost" is the second thing any operator asks, and the first thing any report needs. Pure
prose (`"-100%"`, `"~$180/day"`) cannot be sorted, aggregated, or charted; pure `Option<f64>`
cannot express the honest answer, which in the worked Chullora example is `"N/A (data
quality)"`. Shipping prose-only and adding numbers later is the worst of the three: it builds a
corpus of inconsistently-formatted strings that a later numeric field **cannot be backfilled
from** (nothing can reliably parse `"3.2σ vs baseline"` into value + unit), so the data would
be permanently unqueryable for the window before the upgrade — and that window is where the
first year of real findings live.

So both fields carry a small closed struct:

```rust
pub struct Quantity {
    /// The number, when the producer computed one. Absent = not computed / not applicable.
    pub value: Option<f64>,
    /// Unit of `value` — "%", "kL", "AUD/day", "sigma". Required whenever `value` is set.
    pub unit: Option<String>,
    /// The producer's own words — the honest "N/A (data quality)", or context beside a number
    /// ("vs 1.8 kL baseline"). Always allowed, with or without a value.
    pub note: Option<String>,
}
```

A producer that computed nothing sets `note` only — the "we considered it and it doesn't apply"
signal operators actually read, which a bare omission loses. A producer that computed a number
sets `value` + `unit` and optionally a note. A consumer sorting a roster reads `value` and skips
the rows without one; a consumer rendering a drawer prefers `note`, falling back to formatting
`value`+`unit`. Both readers are simple, and the numeric corpus is correct from the first
record. The cost is one nested type instead of a string — paid once, at scope time, which is
the only time it's cheap.

The other four fields stay plain `Option<String>`: trigger logic, suspected cause, normalised
metric, and benchmark context are irreducibly prose, and nothing will ever sort by them.

**Rejected: reuse the occurrence ring.** Per-firing data has a home
(`occurrence.data`, 2 KB), so "put the analysis on each firing" is tempting. It fails the
consumer test: a drawer wants *the* explanation, not a paginated history of explanations, and
the ring evicts — the reasoning would silently disappear from an old insight while the finding
remained. Analysis is a property of the insight.

**Rejected: an `insight_analysis` table.** Same argument the evidence scope already settled — a
second table for a handful of optional strings read only alongside their parent buys a join and
nothing else.

**Dedup: refreshes on re-raise, like `evidence`.** When a raise supplies `analysis` it
overwrites; when it omits, the stored value is left alone. This deliberately follows `evidence`
rather than `title`/`body`'s first-raise-wins, and for a stronger reason than evidence had: the
whole point of these fields is to describe *the current state of the finding*. A deviation of
"-100%" from firing #1 displayed beside `count: 47` is actively misleading — worse than absent.
(This is precisely the inconsistency `insight-evidence-scope.md` Q1 flagged for `title`/`body`;
this scope resolves it for the new fields and leaves that open question to be decided on its
own merits.)

## How it fits the core

- **Tenancy / isolation:** unchanged — additive fields on the existing `insight:{ws}:{id}`
  record in the workspace namespace. No new key, no new read path, so the shipped isolation
  tests cover it; the mandatory isolation test below re-pins that the new fields don't leak
  cross-workspace via `get`.
- **Capabilities:** **no new capability.** `analysis` is written by the already-gated
  `mcp:insight.raise:call` and read by the already-gated `mcp:insight.get:call`. The evidence
  scope's boundary argument applies verbatim and is worth restating: a producer that may state
  a finding may state its reasoning, and a reader that may read the finding's detail may read
  it. Splitting a cap here would gate prose more tightly than the SQL sitting beside it.
- **Placement:** either. No reactor, no motion, no election — it's a field on a local write.
- **MCP surface (API shape, §6.1):**
  - **CRUD:** no new verb. `insight.raise` gains an optional `analysis` object
    (`RaiseInput.analysis: Option<Analysis>`), refreshed-on-supply per the dedup rule above.
    No `update` — the umbrella's stance holds; correction is another raise.
  - **Get / list:** no new verb. `insight.get` echoes `analysis`. **`insight.list` omits it**,
    for the same two reasons `evidence` is omitted: six prose fields per row would bloat every
    page of a roster for data only the drawer uses, and the roster must stay narrow. This is
    the get-vs-list boundary the evidence scope drew, reused unchanged.
  - **Live feed:** the SSE `RaiseEvent` does **not** carry `analysis` (it carries ids and
    status; a drawer opens with a `get`). Same disposition as evidence Q5.
  - **Batch:** N/A — raise is single and bounded.
- **Data (SurrealDB):** one new optional nested object on the existing `insight` table. No new
  table, no new store, no index. Size-guarded like its neighbours (below).
- **Bus (Zenoh):** nothing new. State is SurrealDB's; the existing fire-and-forget event is
  unchanged.
- **Sync / authority:** ordinary workspace data; nothing analysis-specific.
- **Secrets:** none — but see the risk on prose leakage below.
- **SDK/WIT impact:** **none.** Additive optional fields; an old client deserializes fine and
  an extension reaches `insight.raise` through the unchanged host-callback MCP path.
- **Skill doc:** **YES** — extend `skills/insights/SKILL.md` with an `analysis`-carrying raise
  and what each of the six fields means, grounded in a live run. It must state the closed-struct
  rule explicitly (**unknown keys are dropped; use `body` for anything else**), because that is
  the trap a producer author will otherwise hit silently.

### The shape

```rust
/// Serialized-size cap for the whole `analysis` object. Six short prose fields — a paragraph
/// each, not a report. Exceeding it rejects the WHOLE raise (never silent truncation), the
/// contract `validate_evidence_size`/`validate_occurrence_size` already hold.
pub const MAX_ANALYSIS_BYTES: usize = 4 * 1024;

/// The producer's own explanation of the finding. Every field optional: a producer that knows
/// only its trigger logic still says something useful, and one that knows nothing omits
/// `analysis` whole. CLOSED on purpose — anything outside these six belongs in `body`.
pub struct Analysis {
    /// Why it fired, in the producer's words — "Zero water consumption for 24 consecutive hours".
    pub trigger_logic: Option<String>,
    /// The producer's HYPOTHESIS — "Meter offline or site unoccupied (weekend)". Named
    /// `suspected_cause`, never `root_cause`: a rule that saw one series has not diagnosed
    /// anything, and the field name is the only thing standing between that guess and an
    /// operator who skips a site visit because "root cause" sounded settled.
    pub suspected_cause: Option<String>,
    /// The metric judged, normalised — "Daily water usage (kL)".
    pub normalised_metric: Option<String>,
    /// What it was compared against — "vs expected minimum baseline".
    pub benchmark_context: Option<String>,
    /// How far off — `{ value: -100.0, unit: "%" }`, or note-only "N/A". Sortable by design.
    pub deviation: Option<Quantity>,
    /// Consequence if unaddressed — `{ value: 180.0, unit: "AUD/day" }`, or note-only
    /// "N/A (data quality)". The field reports rank by.
    pub estimated_impact: Option<Quantity>,
}
```

Per-field length is bounded by the object cap only — one guard, checked up front, before any
write, so an oversize payload leaves no orphan parent row.

## Example flow

The Chullora water-meter finding, carrying its own reasoning:

1. A nightly rule judges daily water usage per meter, finds Chullora flat at zero, and raises:
   `insight.raise { dedup_key: "rule:no-water-1d:WM-CHU-01", severity: "warning",
   title: "Chullora — no water usage in 1 day", tags: { building: "chullora-dc",
   asset_type: "water-meter", data_type: "water", classification: "plumbing" },
   evidence: { source: "bms", series: [{ sql: "…daily kL…", unit: "kL" }], threshold: 0.0,
   window: { from, to } }, analysis: { trigger_logic: "Zero water consumption for 24
   consecutive hours", suspected_cause: "Meter offline or site unoccupied (weekend)",
   normalised_metric: "Daily water usage (kL)", benchmark_context: "vs expected minimum
   baseline", deviation: { note: "N/A" },
   estimated_impact: { note: "N/A (data quality)" } } }` — no number computed, but the
   *reason* there isn't one is on the record.
2. The host validates both caps up front, stamps `producer`, writes the record, appends the
   firing to the ring, and fires the matcher + event. No query runs; no field is checked for
   agreement with any other.
3. An operator opens the roster. `insight.list` returns the narrow row — title, severity,
   status, tags, `last_ts` — and **no** analysis or evidence. The page stays cheap.
4. They open the drawer. `insight.get` returns the full record; the drawer renders the six
   labelled fields in a fixed layout above the trend that `evidence` binds. The operator reads
   *why it fired* and *what it means* without opening the rule.
5. The insights-analyst persona, asked "what's wrong at Chullora", reads `analysis` by name and
   answers with the producer's own reasoning instead of paraphrasing `body` JSON.
6. The rule is later improved to compute a real baseline. It re-raises the same key with
   `deviation: { value: -100.0, unit: "%", note: "vs 1.8 kL baseline" }` and
   `estimated_impact: { value: 180.0, unit: "AUD/day" }` — **the stored analysis refreshes**
   (the dedup rule), so the drawer stops showing the old `"N/A"` while the count says 47.
7. A weekly report now ranks the workspace's open findings by
   `analysis.estimated_impact.value` — possible because the number was stored as a number from
   the first record that had one, rather than as prose needing a parser.

## Testing plan

Per `scope/testing/testing-scope.md`, against the **real** store (`mem://`) and a real spawned
gateway — no mocks (rule 9). Mandatory categories:

- **Capability deny (mandatory).** No new cap, so the test is that the **existing** gates still
  cover the new field: a token without `mcp:insight.raise:call` cannot write `analysis`; a token
  without `mcp:insight.get:call` cannot read it. Per the deny-test lesson, assert the property
  only the outer gate has — a real id and a fictional id must produce **identical** errors — and
  **revert-check** the gate rather than trusting an inner layer to fail.
- **Workspace isolation (mandatory).** ws-B's `insight.get` on a ws-A id carrying `analysis`
  returns the same error as for an id that exists nowhere; no prose leaks in the error body.
- **Offline / sync, hot-reload:** N/A (a field on a local record; no extension state).

Key cases:

1. **Round-trip.** Raise with all six fields → `get` echoes them verbatim; raise with one field
   → the other five are absent from the JSON (`skip_serializing_if`), not empty strings.
1a. **`Quantity` in all three shapes.** Note-only (`{ note: "N/A" }`), value+unit, and all three
   together each round-trip; a `value` with **no** `unit` is rejected at raise (a bare number
   whose unit nobody recorded is the seed of the unit-mismatch bug in §Risks); a `Quantity` with
   every field absent is rejected rather than stored as an empty object. Assert `value` decodes
   as a **number**, not a stringified one — the sortability this type exists for dies silently if
   a producer's JSON encoder quotes it, and `serde_json` will happily accept a string into
   `Option<f64>` nowhere, so this is the test that proves the corpus is queryable.
2. **Absent-safe / backward-compatible.** A record written **before** the field existed decodes
   with `analysis: None` — seed the pre-field JSON shape into the real store and read it back.
   This is the one case a fresh-schema test silently skips, and it's the whole "additive" claim.
3. **Dedup refresh.** Raise with analysis → re-raise with *different* analysis → stored value is
   the new one; re-raise with **no** analysis → stored value is left alone (not blanked).
   Revert-check: make the arm unconditional and confirm the omit-case test goes red.
4. **Size guard rejects the whole raise.** An oversize `analysis` errors **before** any write —
   assert no parent row and no occurrence row exist afterwards (the orphan-row contract, not
   just the error). Also assert the message names `body` as the overflow, since that error is
   the producer's only teacher.
5. **`get`/`list` boundary.** `insight.list` never returns `analysis`, with any filter
   combination and across a keyset page boundary; `insight.get` always does when stored.
6. **Closed-struct drop is explicit.** A raise carrying an unknown key inside `analysis` (e.g.
   `confidence`) succeeds and drops it. Pin this deliberately — it's the documented trap, and a
   test that asserts the drop keeps a future contributor from believing the map is open.
7. **Live verification in the product** (not just the suite — `cargo test` has historically not
   caught the real bugs here): raise a real analysis-carrying insight against the running node
   and confirm the drawer renders all six labels and the roster request stays narrow.

## Risks & hard problems

- **The closed struct will be extended, and the extension will fail silently.** This repo's
  most-repeated bug: a new field added at the UI or producer edge is dropped until the Rust type
  learns it, with no error anywhere. Accepted deliberately here (a closed vocabulary is the
  feature), so the mitigation must be real: the drop is pinned by test §6, `body` is documented
  as the overflow in the skill doc, and the struct's doc comment says CLOSED in as many words.
- **A hypothesis will still be read as a diagnosis.** `suspected_cause` is named to fight this
  and the name only gets us part of the way: a confident sentence from a rule that saw one
  series will send someone past a site visit. The UI must carry the hedge in the *label*
  ("Suspected cause"), not only in the field name — the one place this decision can quietly
  evaporate is between this doc and the drawer.
- **Estimated impact is a financial claim with no provenance.** Unlike `evidence`, which points
  at a query you can re-run, an impact figure is unfalsifiable — and now that it's a sortable
  number it will be summed into reports and put in front of customers. Nothing on the record
  says how it was derived. A `body` link to the calculation is the producer's job; the platform
  can't enforce it, and the number's new precision makes it look more authoritative than the
  prose did. This is the cost of typing the field, accepted knowingly.
- **`unit` is unvalidated free text.** Nothing stops one rule writing `"AUD/day"` and another
  `"$/day"`, which breaks exactly the cross-finding aggregation the `Quantity` exists to enable.
  A closed unit enum was rejected (units are domain-open — kL, kWh, σ, currency), so consistency
  is producer discipline enforced by the skill doc. A consumer summing impact across producers
  must group by `unit` and refuse to add unlike units rather than assuming.
- **Prose is a disclosure surface.** `evidence` already carries SQL (schema disclosure — the
  reason it's `get`-only). Analysis carries free text that producers will populate from anything
  in scope, including site names, occupancy, and tenant behaviour. The `get`-only boundary is
  what contains it; anything that later widens `list` to include `analysis` widens that too.
- **Six fields is a guess.** It comes from one worked example (energy/water FDD). A second
  vertical will want a seventh, and the closed-struct decision means that's a core change. If it
  happens twice, the map-vs-struct call should be reopened rather than growing the struct to ten.

## Resolved decisions

Stated here rather than as open questions, so the implementing session has no ambiguity.

1. **`deviation`/`estimated_impact` are `Quantity` (number + unit + note), not strings.**
   Argued in full under "Intent". The deciding factor is that prose **cannot be backfilled into
   numbers** — shipping strings first would leave the first year of findings permanently
   unsortable. A note-only `Quantity` preserves the honest "N/A (data quality)" case that a bare
   `Option<f64>` would have forced producers to drop.
2. **The hypothesis field is `suspected_cause`, not `root_cause`.** Renaming a stored field
   later is a migration, so this is decided now rather than deferred to the UI label. The name
   is the durable part of the hedge; the label carries the rest.
3. **The struct stays closed; `body` is the documented overflow.** A map would hand every
   producer a private vocabulary and lose the fixed drawer layout and the agent-readable field
   names, which are the whole feature. The accepted cost is the silent-drop failure mode — pinned
   by test §6 and documented in the skill doc. **Escalation rule:** if a *second* vertical asks
   for a seventh field, reopen the map-vs-struct call rather than growing the struct toward ten.
4. **`analysis` and `evidence` refresh independently, both on-supply.** Omission means
   "unchanged" for each. A producer that changes its query but not its prose creates real skew,
   and that is the producer's bug to fix, not a reason to couple two fields whose lifetimes are
   otherwise unrelated.
5. **`unit` is free text, not an enum.** Units are domain-open (kL, kWh, σ, currency-per-day);
   a closed enum would either be wrong or grow forever. Consumers aggregating across producers
   must group by `unit` and refuse to sum unlike units.
6. **`title`/`body` stay first-raise-wins — and that is now a known bug with a named owner.**
   This scope resolves the refresh question for its own fields and *deliberately does not* change
   `title`/`body` (out of scope, and a behaviour change to shipped semantics). But a record with
   fresh `analysis` beside a firing-#1 `body` is a new, visible inconsistency, and `analysis`
   existing is the reason the old behaviour becomes indefensible: the drawer will show correct
   reasoning above a stale narrative. **This closes `insight-evidence-scope.md` Q1 as
   "decided elsewhere": `title`/`body` should refresh on re-raise, in its own small scope, and
   that scope is now a follow-up of this one rather than an open musing.** The implementing
   session should file it, not fix it inline.
7. **The insights-analyst persona's grounding skill is updated in the same session** that ships
   the field — a persona still reasoning over `body` while named fields exist is the stale-skill
   finding `SCOPE-WRITTING` §6 warns about, not a nicety.

## Open questions after building

All seven resolved decisions held as written; decision 3's drop and decision 4's independence are
pinned by tests, and the dedup arm is **revert-checked**. Three notes the scope did not anticipate:

1. **The guard is `validate_analysis`, not `validate_analysis_size`.** Two of the scope's demanded
   rejections (test §1a) are *shape* rules, not size — a `value` with no `unit`, and an all-absent
   `Quantity`. One guard enforces both plus the cap, called once at the raise boundary; the `_size`
   name would have lied about what it checks. **Closed** (a naming decision, made and recorded).
2. **The producer doors needed no change at all.** Every door — the rhai handle, the flow sink, the
   MCP verb — funnels through the single `RaiseInput` deserialization in
   `host/src/insight/tool.rs:44`, so `analysis` reached all of them for free. The scope's
   "SDK/WIT impact: none" claim held literally, and `rule-raises-insight-scope.md`'s "must carry
   `analysis` through unchanged" needed no work. **Closed.**
3. **Scope §7's drawer check cannot be done in this repo.** The six-label layout, and the hedge in
   the *label* ("Suspected cause") that §Risks says must not evaporate, are a downstream `rubix-ai`
   change — lb is a library and the shell is out-of-tree (`MIGRATION.md`). The live run verified the
   field arrives at the consumer boundary with the right shape (incl. `value` decoding as a number
   over the wire), which is as far as this repo reaches. **Open downstream**, not open here.

Decision 6 is discharged by **filing** [`insight-prose-refresh-scope.md`](insight-prose-refresh-scope.md)
rather than fixing `title`/`body` inline, as instructed.

## Related

- **The follow-up this scope creates:**
  [`insight-prose-refresh-scope.md`](insight-prose-refresh-scope.md) — `title`/`body` should refresh
  on re-raise. Filed by the implementing session per resolved decision 6; `analysis` existing is what
  makes the old first-raise-wins behaviour indefensible.
- Parent: [`insights-scope.md`](insights-scope.md) (the record, the tag rule, §"MCP surface")
- **Direct sibling / the other half:** [`insight-evidence-scope.md`](insight-evidence-scope.md)
  — the data binding this reasoning sits beside; its size-guard, `get`-vs-`list` boundary,
  refresh-on-re-raise dedup rule, and no-new-capability argument are all reused here, and its
  Q1 is **closed as "decided elsewhere"** by this scope's resolved decision 6.
- **The dimension columns:** [`insight-tag-echo-scope.md`](insight-tag-echo-scope.md) — echoes
  the tag facets onto the record so a roster renders building / asset type / priority /
  classification from one `list` call. The other half of "more fields on an insight".
- Sibling: [`insight-triage-scope.md`](insight-triage-scope.md) — the **human** plane (ownership
  + comments) this deliberately is not; together the two split the ~22 requested fields into
  tags / `dedup_key` / analysis / triage.
- [`insights-package-scope.md`](insights-package-scope.md) — the `lb-insights` crate the type
  lands in (`analysis.rs`, one responsibility: the shape + its size guard).
- [`insight-occurrences-scope.md`](insight-occurrences-scope.md) — the per-firing ring and the
  2 KB cap that models the size guard.
- [`rule-raises-insight-scope.md`](rule-raises-insight-scope.md) — the rhai producer door that
  must carry `analysis` through unchanged.
- `scope/tags/tags-scope.md` — where the dimension fields go instead.
- `skills/insights/SKILL.md` — extended by the implementing session; must document the
  closed-struct rule and `body` as the overflow.
- README §3 (rules 5–7), §6.5.
