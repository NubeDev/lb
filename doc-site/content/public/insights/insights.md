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
