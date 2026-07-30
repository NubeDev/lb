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

## Triage — who owns it, and what we found out

The record answered *what fired* and *who last moved the status*. It could not answer **"who owns
this"** or **"what did we find out"** — so operators triaged in a spreadsheet beside the app. The
triage plane adds the human half: one owner axis and an append-only note thread.

```jsonc
// insight.get — the drawer gets the record AND the whole thread
{
  "id": "01KYR…", "dedup_key": "rule:no-water-1d:WM-CHU-01", "status": "open",
  "assigned_to": "user:priya",
  "comments": [
    { "cseq": 2, "text": "Facilities confirmed the shutdown.", "author": "user:priya", "ts": … },
    { "cseq": 1, "text": "Site shut for the long weekend?",     "author": "user:ada",   "ts": … }
  ]
}
```

Two verbs, each with its **own capability** — `insight.assign` (`mcp:insight.assign:call`) and
`insight.comment` (`mcp:insight.comment:call`), both member-act grade beside `ack`/`resolve`:

```bash
insight.assign  { id | ids[≤100], assignee? }   # assignee: null clears; team: subjects legal
insight.comment { id, text, ts }                # → { seq }
insight.list    { assigned_to: "me" | "none" | "user:…" | "team:…" }
```

There is deliberately **no `insight.update`**. One verb for "change any field" would mean a
producer-grade `insight.raise` grant also buys the power to rewrite human triage state, and the deny
path would stop being expressible. Two narrow verbs keep one capability per capability.

### The rule that makes it trustworthy

**A re-raise never touches either field — including the re-open arm.** When a resolved finding fires
again, `status_by`/`status_ts` clear (a fresh lifecycle) but `assigned_to` and the thread do **not**:
the fault came back and it is still Priya's, and the note explaining last time's false alarm is the
most valuable thing on the record at that moment. A flapping sensor re-firing every 15 minutes can
never silently un-assign the technician who took the job — there is no `assigned_to` on the raise
input at all, so no producer can reach the plane.

### The rules that come with it

- **`assigned_to` is a subject, not a user id.** `user:priya` and `team:mechanical` are both legal
  from v1, because queue-style ownership is how real triage works and retrofitting `team:` later
  would retroactively break every consumer that parsed the field as a user. The assignee is
  **validated** at assign time (it must be a member or team of this workspace) — and a subject from
  another workspace is refused with the **same opaque error** as one that doesn't exist, so assign is
  never a cross-tenant existence oracle.
- **`assigned_to` rides `insight.list`; `comments` never does.** The boundary rule is again *"does a
  column need it"* — the assignee is the 6th column operators ask for, and the thread is the first
  thing that would make every roster page expensive. The thread is `get`-only.
- **Comments do not evict.** This is the one place the thread diverges from the occurrence ring whose
  storage shape it reuses, and it is deliberate: eviction is right for firings (machine-generated,
  individually low-value) and a **trust failure** for human notes. Both bounds refuse instead — a
  comment over 4 KB rejects the call, and appending past 200 comments errors with the existing thread
  **unchanged**. A thread that long means the finding should have become a work item, and the
  platform says so rather than quietly deleting the oldest note. Comments are purged only *with*
  their insight.
- **The author is host-stamped.** A comment's `author` is always the calling principal — a supplied
  one is ignored, the same discipline `status_by` has. The thread is append-only: there is no edit or
  delete in v1, so a correction is another comment and both remain.
- **Bulk assign reports, never truncates.** Up to 100 ids with **per-item results**
  (`{results:[{id, ok, error?}]}`); more than 100 is an explicit error and nothing is assigned. A UI
  must surface the failures — a green toast over 12 silently failed rows is the same bug in a
  friendlier costume.

**Known gap — assigning notifies nobody.** The subscription ladder is subject-matched, not
assignee-matched, so v1 assignment is a roster fact and the assignee is **not** paged. A UI must not
imply a notification was sent. An `assignee` match arm is the sequenced follow-up.

**Known gap — a subject outlives its membership.** A member removed from the workspace leaves
insights assigned to a subject that can no longer read them. The record deliberately keeps the stale
value rather than guessing an heir; render an unresolvable assignee as *"unknown (removed)"*, never
blank, so the orphaned queue is visible rather than silently empty.

Scope + rationale (including why comments are not a ring):
[`insight-triage-scope.md`](../../../../docs/scope/insights/insight-triage-scope.md).
