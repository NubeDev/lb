# Insights scope — `evidence`: the finding states the data that proves it

Status: **BUILT — awaiting PR + `node-v*` tag (2026-07-19).** Implemented behind this scope:
`rust/crates/insights/src/evidence.rs` (the shapes + the 4 KB guard), threaded through
`insight.rs`/`raise.rs`/`list.rs`, `packages/insights` (types + `memoryClient`), and the `bas`
fixture rules. Tests: `rust/crates/host/tests/insight_evidence_test.rs` (10, incl. the migration
guard, dedup-refresh, and the get-vs-list boundary) — green, and the full `lb-host` suite is green.
Verified live on a real node from the downstream consumer: a building-level finding now raises with
its own binding, the stated series executes and returns plottable rows, `insight.list` omits the
descriptor, and the trend draws in the browser. Promotes to `doc-site/content/public/insights/`
once tagged.
Owning repo: **`NubeDev/lb`** (this repo) — the record, the raise seam, the read verbs, and the
`@nube/insights` types. Consumed downstream by `NubeIO/rubix-ai` (`docs/scope/fdd/fdd-insight-viz-scope.md`
§V3) on a `node-v*` tag bump.

An insight says *what* went wrong and *when*, but nothing machine-readable about **what data proves
it**. A rule runs a query, judges the result, and raises — and the query, the one artefact that ties
the finding to the trend an operator wants to see, is discarded at the raise boundary. Every consumer
that wants to draw "the spark on the chart" is therefore forced to guess: reconstruct a series from
whatever the producer happened to stuff in `body`, hardcode a datasource, and hope. That guess is
wrong for whole classes of rule and silently wrong for the rest. This scope adds one optional field —
`evidence` — so the producer states its own binding, and every consumer reads it instead of inferring.

## Goals

- An optional **`evidence`** on `insight.raise`, persisted on the record and echoed by `insight.get`:
  the datasource + the plottable series (one or more) the finding sits on, plus the threshold and
  window it judged against.
- **Consumers stop guessing.** A trend viewer binds a panel straight from `evidence` — no
  body-shape sniffing, no hardcoded datasource id, no per-rule special case.
- **Additive and safe.** Absent on every existing record and every producer that doesn't set it;
  no record fails to decode, no read verb changes shape for a caller that ignores it.
- **Generic.** The field's vocabulary is datasource + query, the same vocabulary the data plane
  already speaks. No FDD term, no rule-family term, no consumer named anywhere (rule 10).

## Non-goals

- **Not a query executor.** The node does not run, validate, or plan the `evidence` query at raise
  time. It is a *descriptor*; executing it is the reader's business, through `federation.query` and
  that verb's own caps and workspace wall. A malformed series is a broken panel, not a failed raise.
- **Not a replacement for `body`.** `body` stays the free-form human/agent-readable detail. `evidence`
  is the narrow machine-readable binding beside it. Producers that only want prose keep using `body`.
- **Not the occurrence ring.** Per-firing evidence rows already have a home (`occurrence.data`,
  2 KB cap, its own capability). `evidence` describes the *series*, which is a property of the
  insight, not of one firing.
- **Not tag echo.** A sibling gap (see §Related) — out of scope here.
- **No new capability** in v1 (§How it fits argues the boundary).

## Intent / approach

### The shape

`evidence` is an optional object on `RaiseInput` and on the persisted `Insight`:

```rust
pub struct Evidence {
    /// Datasource id the series resolve against (e.g. a federation source name).
    pub source: String,
    /// The plottable series the finding sits on — each yields (time, value) rows.
    /// Serde-untagged: a bare string is sugar for a single unlabelled series.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub series: Vec<EvidenceSeries>,
    /// The judgment query — what the rule actually ran to decide. Provenance/"open evidence" only;
    /// frequently NOT plottable (see below). Optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// The time window judged, as epoch-ms — lets a viewer open pre-ranged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<EvidenceWindow>,   // { from: u64, to: u64 }
    /// The threshold crossed, in the series' own units — drawn as a threshold line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    /// Tool the series dispatch through. Defaults to `federation.query`; present so a producer
    /// on another data plane is not locked out. Never a consumer id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
}

pub struct EvidenceSeries {
    pub sql: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}
```

`series` is a **list** because findings are routinely multi-series: a cooling-failure rule judges
supply *and* return temperature, and the trend that proves it must draw both. A list also maps 1:1
onto the consumer's existing `sources[]` panel vocabulary, so binding is a `map`, not a merge.

### The decisive design point: the judgment query is usually not plottable

The instinct is "persist the SQL the rule ran." That is insufficient, and the shipped rule pack
proves it. `fdd-energy-intensity.rhai` judges with:

```sql
SELECT s.name AS building, ROUND(SUM(pr.value) / …, 2) AS kwh_per_m2
FROM point_reading pr JOIN … GROUP BY s.id, s.name, a.val ORDER BY kwh_per_m2 DESC
```

That is an aggregate: one scalar per building, **no time axis**. Plotting it yields nothing. The
trend an operator wants is the underlying `Energy kWh` series for that one building over time — a
*different query*, which the rule can write but the engine cannot derive. Hence the split: `series`
is what you draw, `query` is what was judged, and only `series` is required for a viewer to work.

Single-point rules (`fdd-sensor-flatline`) collapse the two — their series *is* their evidence — but
designing for the aggregate case is what keeps this from being a half-fix.

### Rejected alternative: auto-capture the rule's queries

The engine could record every `query(source, sql)` a rule ran during the run and attach them
automatically — zero authoring burden, and tempting. Rejected for two reasons: a rule runs many
queries and the engine cannot know which one proves the finding (nor which *row* of it, for a rule
that raises per-row); and, per above, the captured query is frequently the non-plottable aggregate.
Auto-capture would produce confidently-wrong bindings, which is worse than the honest empty state we
have now. Explicit beats inferred here. (Auto-capture remains a reasonable *future* nicety for
populating `query` as provenance — it is the plottable `series` that must stay authored.)

### Rejected alternative: leave it in `body`, by convention

This is the status quo, and it is what the downstream consumer is currently doing — sniffing
`body.point` and hardcoding the datasource. It fails the moment a rule has no single point, and it
puts a wire contract in an explicitly free-form field. `body`'s doc comment already claims the
evidence role ("evidence rows, scores, links") without any of the guarantees a consumer needs; this
scope makes the machine-readable part of that promise real and leaves the prose part where it is.

### Rejected alternative: a separate `insight_evidence` table

An extra read on the common path to save bytes on a field that is a few hundred bytes and always
wanted alongside the record. The parent record is already a JSON blob; one more optional key is the
cheaper shape. Revisit only if evidence grows to hold sample data (it must not — that is the
occurrence ring's job).

## How it fits

**Dedup — the sharp edge.** `raise()` mutates only `count`, `last_ts`, `severity`, and possibly
`status` on a re-raise; `title`, `body`, and `origin` from the second raise are **discarded**
(first-raise-wins). Evidence must **not** inherit that: it is a *binding*, not a historical fact, and
a rule edited to query a renamed table would otherwise leave every existing insight bound to a query
that no longer runs, permanently, with no way to heal short of deleting the record. **Decision:
`evidence` is overwritten on every raise that supplies one; a raise that omits it leaves the stored
value untouched** (so a producer can stop sending it without blanking the binding). This is a
deliberate divergence from `body`'s behavior and must be commented as such at the mutation site.
Whether `title`/`body` should get the same treatment is a real question but a separate behavior
change — see Open questions.

**Migration.** Records are JSON blobs under a `data` envelope with no schema version, and both
`list` and `occurrences` decode with `filter_map(|v| from_value(v).ok())` — **a record that fails to
decode is silently dropped from the list**. A required field would therefore make every
pre-existing insight vanish from `insight.list` with no error anywhere. `evidence` is
`Option<Evidence>` + `#[serde(default)]`; this is not a style preference, it is the whole migration
story. No `deny_unknown_fields` exists in the crate, so the wire tolerates the field before and
after it lands in Rust. No heal pass is needed (contrast `heal_ts.rs`, which existed because a
*value* was wrong, not a shape).

**Capabilities and the deny path.** No new capability. But the boundary needs stating, because the
crate already draws one: occurrences sit behind their own `mcp:insight.occurrences:call` precisely
because "the list/get read cap does NOT imply evidence access". `evidence` carries a datasource id
and SQL — table and column names, i.e. schema disclosure — so putting it on the parent widens what
an `insight.list` holder learns. **Decision: `evidence` is echoed by `insight.get` and omitted from
`insight.list`.** The reasons compound: list pages are many-record and evidence would bloat every
page for a field only the detail view uses; and the narrower read is the one that already implies
"you are investigating this one finding". Holding the descriptor grants nothing — running the query
still goes through `federation.query`, its own caps, and the workspace wall. Deny path is unchanged:
a producer without `mcp:insight.raise:call` is denied opaquely before any of this.

**Workspace isolation.** Unchanged — evidence rides the record, which is already ws-scoped; the
`source` it names resolves per-workspace at read time by the data plane, not at raise.

**Size.** `body` has no cap today (arguably a gap); `occurrence.data` caps at 2 KB and rejects the
whole raise. `evidence` gets a **4 KB cap** in the same style and the same place (a validate step
before any write), sized for a few joins' worth of SQL. The error message should name the field and
point at the ring, mirroring the existing one.

**Symmetric nodes.** Nothing role-dependent; a gateway and a reactor persist and echo evidence
identically.

**No mocks.** Tests raise through the real crate verb against a real `mem://` store and read back
through the real `insight.get`, per rule 9.

**The rhai seam needs no change.** `verbs/insight.rs` builds its input via `map_to_json(&item)`, a
generic passthrough — an author-supplied `evidence` key already flows untouched to the MCP verb,
where `RaiseInput`'s serde silently drops it today. That is exactly why this is a one-struct change
and not a seam change, and it means rule authors can adopt it the moment the field lands.

**SDK/UI impact.** `packages/insights/src/types.ts` (`Insight.evidence?`) and `memoryClient.ts` must
be threaded — the package states its contract as mirroring the wire one-to-one, so a divergence here
is a bug. Flagged loudly because `dist/` is committed and consumers see stale types otherwise.

## Example flow

1. `fdd-energy-intensity.rhai` queries `demo-buildings` for per-building kWh/m², judging against a
   budget of 1.0. `Northside Factory` comes back at 2.4.
2. The rule raises, now naming its evidence — the *plottable* series, not the aggregate it judged:
   ```rhai
   insight.raise(#{
     dedup_key: "energy-intensity-high:" + building,
     severity: "critical",
     title: building + " energy intensity high",
     body: #{ building: building, kwh_per_m2: intensity, budget: 1.0 },
     evidence: #{
       source: "demo-buildings",
       series: [#{ sql: "SELECT pr.time, pr.value FROM point_reading pr JOIN point p ON p.id = pr.point_id JOIN meter m ON m.id = p.meter_id JOIN site s ON s.id = m.site_id WHERE s.name = '" + building + "' AND p.name = 'Energy kWh' ORDER BY pr.time",
                   label: "Energy kWh", unit: "kWh" }],
       threshold: 1.0,
     },
     tags: #{ area: "energy", building: building },
   });
   ```
3. The rhai handle passes the map through, injects `ts`/`origin`, and calls `insight.raise`.
4. The host authorizes, force-stamps `producer`, normalizes `ts`. The crate validates the evidence
   size, then upserts by `(ws, dedup_key)` — first raise creates, later raises bump `count`/`last_ts`
   **and refresh `evidence`**.
5. An operator opens the finding. `insight.get` returns the record **with `evidence`**.
6. The consumer maps `evidence` to a panel target — `{tool: "federation.query", args: {source, sql}}`
   per series — draws the trend, puts the threshold line at 1.0, and overlays the raise markers from
   `insight.occurrences`. The chart shows the line crossing the budget and the marker where it fired.
7. `insight.list` on the same workspace returns the record **without** `evidence` — the roster is
   unchanged in size and shape.

## Testing plan

Mandatory categories:
- **Capability-deny:** a principal without `mcp:insight.raise:call` is denied opaquely with
  `evidence` present exactly as without it (evidence must not open an alternate path); a principal
  with `insight.list` but not `insight.get` never receives an evidence payload.
- **Workspace isolation:** an insight raised with evidence in `ws-a` is invisible (record and
  evidence) to `ws-b`; the `source` name is not resolved or leaked cross-workspace.
- **Offline/sync, hot-reload:** N/A — no motion surface and no reload path is touched.

Key cases:
- **Round-trip:** raise with full evidence → `insight.get` echoes it byte-identically.
- **The migration guard (the one that matters):** seed a record written *without* the field (the
  pre-existing blob shape), then `insight.list` — it must still appear. This is the regression that
  would silently empty every roster, so it gets an explicit test, not a incidental one.
- **Absent/partial:** raise with no `evidence` (record has none, no serialization noise); with
  `source` + one bare-string series (untagged sugar decodes); with `series` omitted but `query` set.
- **Dedup refresh:** raise twice with *different* evidence → the stored record carries the **second**;
  raise a third time with `evidence` omitted → the second's value survives. Asserted alongside the
  existing "body/title do not change" assertion so the divergence is documented in the test.
- **Size cap:** evidence just over 4 KB rejects the **whole** raise (no partial write, `count`
  unchanged, no occurrence appended) with an error naming the field.
- **List omission:** `insight.list` items carry no `evidence` key while `insight.get` on the same id
  does.
- **Rhai end-to-end (real path, no mocks):** run the `bas` fixture pack against a real store and
  assert the raised record's evidence — the fixture rules gain `evidence` blocks as part of this
  work, which doubles as the authoring example.
- **Package parity:** a type-level test that `@nube/insights`' `Insight` and `memoryClient` carry the
  field, so the "one shape, not two" claim stays true.

## What the build taught (2026-07-19)

Three things the scope did not anticipate, recorded because they change the shape:

1. **`Insight` can no longer derive `Eq`.** `threshold` is an `f64` — the honest type for a real
   quantity in the series' own units — and `f64` is not `Eq`. `Insight`, `RaiseInput`, and
   `ListPage` dropped to `PartialEq`. Nothing in the workspace required `Eq` (a full
   `cargo check --workspace` is clean), so the blast radius was nil, but it is a genuine API
   change worth a line in the release note.
2. **A threshold is often NOT comparable to its own series, and the fixture rules prove it.**
   Both rules that gained `evidence` deliberately omit `threshold`: `fdd-sensor-flatline` judges a
   *spread* (max−min < 0.1 °C), which is not a value on the temperature series, and
   `fdd-energy-intensity` judges a whole-period intensity while its series is per-day. Drawing
   either as a line would compare two different quantities. The field's doc says "in the series'
   own units" and that constraint bites more often than expected — which is an argument for it
   staying optional, and against ever defaulting it.
3. **The `series`-is-not-`query` split is load-bearing, confirmed against real data.**
   `fdd-energy-intensity`'s judgment SQL returns exactly one row (`Northside Factory / 2.4`) with
   no time axis; the per-day series it now states returns plottable `(time, value)` rows through
   `federation.query`. Had the field persisted "the SQL the rule ran", this finding would still
   draw nothing.

## Risks & hard problems

- **The silent-drop failure mode.** Covered above and by a named test, but it deserves repeating: the
  `filter_map(ok())` read path converts a schema mistake into missing data with no error. Any
  reviewer of this change should check optionality first and everything else second.
- **SQL embedded in a durable record ages.** A renamed table leaves a stale binding. The dedup-refresh
  decision is the mitigation (the next raise heals it), but an insight whose rule was *deleted* keeps
  a query nobody will fix. Acceptable: a broken panel with a visible error beats a wrong panel, and
  the record remains readable.
- **String interpolation into the series SQL** is on the rule author (the example above concatenates
  a building name). This is not a new exposure — rules already build their judgment SQL the same way
  and run it — but it now gets *persisted*, so a quoting bug becomes durable rather than transient.
  Worth a note in the skill doc.
- **Producers may over-stuff evidence** to dodge the occurrence ring's 2 KB cap. The 4 KB cap and the
  "descriptor, not data" framing in the docs are the guard; if it is abused, the cap tightens.
- **Two consumers could diverge** on how they turn evidence into a target (which tool, how `window`
  applies). Mitigated by specifying the mapping in the public doc rather than leaving it to each
  reader.

## Open questions

1. **Should `title`/`body` also refresh on re-raise?** Today they are first-raise-wins, which means a
   finding's displayed numbers are the *oldest* ones — arguably already a bug (the count says ×5 but
   the body describes firing #1). Deliberately out of scope here, but this scope's dedup decision
   makes the inconsistency explicit and someone should decide it.
2. **Should `window` be relative rather than absolute?** Absolute epoch-ms pins the viewer to the
   moment judged, which is right for provenance; a rule that fires nightly would rather say "24h" so
   the panel follows. Possibly both (`window: {from,to}` and a `relative: "24h"`), possibly the
   consumer's dashboard range wins. Recommend shipping absolute and letting the consumer override.
3. **Does `threshold` belong here or in `fieldConfig` at the consumer?** It is genuinely the rule's
   knowledge, so here — but a multi-series evidence with per-series thresholds would need it on
   `EvidenceSeries` instead. Ship it flat; move it if a real rule needs per-series.
4. **Is 4 KB the right cap?** Chosen by analogy to the 2 KB occurrence cap and a guess at join-heavy
   SQL. Cheap to raise later, expensive to lower — so err low and see what the packs need.
5. **Should `insight.watch`/the SSE `RaiseEvent` carry evidence?** Currently it carries ids and
   status only, and a live-updating panel already has the binding from its cell. Recommend no.

## Related

- [`insights-scope.md`](insights-scope.md) — the umbrella: the record and its producer doors.
- [`insights-package-scope.md`](insights-package-scope.md) — the `lb-insights` crate this field lands in.
- [`insight-occurrences-scope.md`](insight-occurrences-scope.md) — the per-firing ring; its
  "`insight.raise` (existing verb, additive field)" is the exact precedent this follows, and its
  2 KB cap is the model for the size guard.
- [`rule-raises-insight-scope.md`](rule-raises-insight-scope.md) — the rhai producer door that
  carries `evidence` through unchanged.
- **Downstream consumer:** `NubeIO/rubix-ai` → `docs/scope/fdd/fdd-insight-viz-scope.md` §V3, which
  raised this ask and inherits the binding on a `node-v*` bump.
- **Sibling gap (not this scope):** `tags` are persisted to the tag graph and drive `insight.list`
  facet filtering, but are **not echoed** on the `Insight` record — so a UI cannot display or
  client-side filter them without a separate `tags.find`. Worth its own small scope.
