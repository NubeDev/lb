# Insights

An **insight** is a persisted, queryable data finding — raised by a rule, a flow, or an agent —
carrying severity, provenance, dedup-keyed occurrence counting, and an `open → acked → resolved`
lifecycle. Full verb reference: [`docs/skills/insights/SKILL.md`](../../../../docs/skills/insights/SKILL.md);
the scopes (umbrella, occurrences, subscriptions, notify, the rule producer door) live under
[`docs/scope/insights/`](../../../../docs/scope/insights/README.md).

## Tag echo — dimension columns on the roster

Tags are an insight's **dimension plane** (building, asset type, data type, priority, category,
classification). They live in the tag graph, and `insight.list { tags: {…} }` filters through it.
Since the tag echo shipped, the resolved facets are also **on the record**:

```jsonc
// one insight.list call — every row carries its dimensions
{ "items": [ {
    "id": "01KYR…", "dedup_key": "rule:intensity:meter-1", "severity": "warning",
    "status": "open", "count": 2, "last_ts": 1785372228873,
    "tags": { "building": "chullora-dc", "asset_type": "water-meter", "priority": "medium" }
} ] }
```

`tags` is echoed by **both `insight.get` and `insight.list`** — deliberately unlike `evidence`,
which is `get`-only. The boundary rule is *"does the roster render it"*: dimensions exist to be
columns, so a roster draws them from the list response alone — no follow-up `tags.find` per row, and
**no tag capability**. A viewer holding only `mcp:insight.list:call` gets the dimensions; before the
echo, the same roster additionally needed `mcp:tags.of:call`.

### The rules that come with it

- **The graph is the source of truth; the echo is a read-only projection.** It is written only by
  the raise path, from the insight's full facet set in the graph — so it is the **union across all
  raises** of that `dedup_key`, not one firing's declaration. A producer that stops sending
  `classification` does not blank the column.
- **Never caller-writable.** Like `producer`, it is host-computed; a `tags` value on the record in a
  raise body is ignored. Tags are applied at raise (`Source::Producer` provenance) or through the
  `tags.*` verbs — one writer for one truth. There is no `insight.tag` verb.
- **Filtering still reads the graph, never the echo.** `insight.list { tags }` resolves through the
  tag graph, so a filter is correct even while a record's echo is briefly behind — e.g. a tag
  applied out-of-band with no subsequent raise. Display may lag; queries may not.
- **It self-heals.** The echo is recomputed on every raise, so an out-of-band tag change lands on
  the record the next time the finding fires.
- **Dimensions only.** The echo inherits the tag plane's cardinality rule: per-asset identity
  (`WM-CHU-01`) belongs in `dedup_key`. An absurd facet set exceeds the echo's 2 KB cap, and the
  echo is then **skipped whole with a warning** — never silently truncated, and never a failed
  raise (the record and the graph are already correct).
- **Additive.** Absent on every record raised before the field landed, and on records whose
  producer states no tags. A reader that ignores it is unaffected.

**Known gap:** a record raised before the echo shipped stays blank until its next raise —
permanently, if the finding is resolved and never fires again. The backfill job is the sequenced
follow-up; until it lands, render a blank echo as *"no dimensions recorded"*, not as authoritative.

Scope + rationale (including why filtering deliberately does not read the echo):
[`insight-tag-echo-scope.md`](../../../../docs/scope/insights/insight-tag-echo-scope.md).

## `analysis` — the finding explains itself

`evidence` says **where the data is**. `analysis` says what the producer **concluded** — an optional,
closed set of six fields beside it, so a detail drawer renders a stable labelled layout and an agent
reads named fields instead of guessing at free-form `body` JSON.

```jsonc
// insight.raise — and insight.get echoes it back verbatim
"analysis": {
  "trigger_logic":     "Zero water consumption for 24 consecutive hours",
  "suspected_cause":   "Meter offline or site unoccupied (weekend)",
  "normalised_metric": "Daily water usage (kL)",
  "benchmark_context": "vs expected minimum baseline",
  "deviation":         { "value": -100.0, "unit": "%", "note": "vs 1.8 kL baseline" },
  "estimated_impact":  { "value": 180.0,  "unit": "AUD/day" }
}
```

Every field is optional — a producer that knows only its trigger logic states one field, and one that
knows nothing omits `analysis` whole. **No new capability**: it is written by the already-gated
`insight.raise` and read by the already-gated `insight.get`.

### `deviation` and `estimated_impact` are quantities, not prose

Both carry `{ value?, unit?, note? }` rather than a string, because these are the two fields reports
**rank by** — "show me today's findings by cost". Prose cannot be sorted, aggregated, or charted, and
critically it **cannot be backfilled into numbers** later (nothing reliably parses `"3.2σ vs
baseline"`), so a prose-first field would have left the first year of findings permanently
unqueryable. The `note` keeps the honest answer available: `{ "note": "N/A (data quality)" }` is a
producer saying *we considered this and it does not apply* — which a bare number would have forced it
to drop, and which a plain omission loses.

Two shapes are refused outright: a `value` with **no `unit`** (an uninterpretable number that breaks
the very aggregation the type exists for), and an all-absent `{}`. `unit` is free text, so a consumer
summing impact across producers must **group by `unit`** and refuse to add unlike units — nothing
stops one rule writing `"AUD/day"` and another `"$/day"`.

### The rules that come with it

- **`get`-only — `insight.list` never returns it.** Six prose fields per row would bloat every page of
  a roster for data only the drawer uses, and that boundary is also what contains free text producers
  populate from anything in scope (site names, occupancy, tenant behaviour). Note the deliberate
  asymmetry with the tag echo above: the rule is *"does a column need it"* — tags exist to be columns,
  `analysis` exists to fill a drawer.
- **The struct is CLOSED, and a seventh field is dropped silently.** Deliberate: a fixed vocabulary is
  the whole feature, since a free map hands every producer a private one (`root_cause` vs `rootCause`
  vs `cause`) and loses both the fixed layout and the agent-readable names. `body` remains the
  documented overflow for anything else. The trade-off is real and worth knowing before you author a
  producer: a key outside the six will never error and never store.
- **`suspected_cause`, never `root_cause`.** A rule that saw one series has not diagnosed anything.
  The field name is the durable half of that hedge; a UI must carry the other half in the **label**.
- **Nothing is computed or verified by the node.** These are the producer's claims, stored verbatim —
  the node never checks that `deviation` agrees with `evidence.threshold` and never runs a query to
  fill one in, the same posture `evidence` takes on SQL it never executes. In particular
  `estimated_impact` is unfalsifiable: unlike `evidence`, which points at a query you can re-run,
  nothing on the record says how the figure was derived. Attribute it; don't assert it.
- **Refreshes on supply.** A raise that supplies `analysis` overwrites the stored value; a raise that
  omits it leaves it alone. Refreshing matters here more than for `evidence`: a `"-100%"` deviation
  computed at firing #1 displayed beside `count: 47` is worse than absent. `analysis` and `evidence`
  refresh independently of each other.
- **Guarded up front.** An `analysis` over 4 KB — or either malformed quantity — rejects the **whole
  raise** before any write, so a bad payload never leaves an orphan record. Never a silent truncation.
- **Additive.** Absent on every record raised before the field landed. A reader that ignores it is
  unaffected, and no verb changed shape for an old client.

**Known inconsistency:** a re-raise refreshes `analysis` but still freezes `title` and `body`
(first-raise-wins), so a long-lived finding can show fresh reasoning above a firing-#1 narrative.
Tracked as [`insight-prose-refresh-scope.md`](../../../../docs/scope/insights/insight-prose-refresh-scope.md).

Scope + rationale (including why the struct stays closed):
[`insight-analysis-scope.md`](../../../../docs/scope/insights/insight-analysis-scope.md).
