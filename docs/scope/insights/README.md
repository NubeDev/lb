# Insights scope — index

The **insight** is the one durable, queryable data-finding record (severity + provenance +
entity tags + `open → acked → resolved` lifecycle with dedup) over the shipped
rules/flows/attention planes. Start with the umbrella; the rest compose onto it.

- [`insights-scope.md`](insights-scope.md) — **the umbrella**: the record, the three producer
  doors (rule handle, flow sink node, MCP verb), the two consumer surfaces, and the page.
- [`insights-package-scope.md`](insights-package-scope.md) — the `lb-insights` crate (record
  types, `raise`/dedup, occurrence append) the host verbs ride.
- [`insight-occurrences-scope.md`](insight-occurrences-scope.md) — the per-insight transaction
  ring: every raise appends one size-capped occurrence row (last N).
- [`insight-evidence-scope.md`](insight-evidence-scope.md) — **the finding states its own data**: an
  optional `evidence` on raise (datasource + the plottable series + threshold/window), persisted and
  echoed by `insight.get`, so a trend viewer binds from the record instead of guessing a series out
  of `body`. Decides dedup-refresh (evidence is a binding, not history) and the get-vs-list boundary.
- [`insight-tag-echo-scope.md`](insight-tag-echo-scope.md) — **the record carries its own
  facets**: tags are persisted to the graph and filter `insight.list`, but aren't *on* the record,
  so a roster can't render building / asset type / priority as columns without an N+1
  `tags.find`. Echoes the materialized facet set (a value the raise path already computes for
  subscription matching and discards) onto the record, on **both** `get` and `list`. Read-only
  projection; the graph stays the write path and the filter path. **Ship this first** — the other
  two assume it.
- [`insight-analysis-scope.md`](insight-analysis-scope.md) — **the finding explains itself**: the
  other half of `evidence`. An optional closed `analysis` struct (trigger logic, suspected cause,
  normalised metric, benchmark context, deviation, estimated impact) so a drawer renders a stable
  labelled layout and the analyst persona reads named fields instead of sniffing `body`.
  `get`-only, refreshes on re-raise, no new cap. `deviation`/`estimated_impact` are a
  **`Quantity`** (number + unit + note), not prose — prose can't be backfilled into numbers.
- [`insight-triage-scope.md`](insight-triage-scope.md) — **the human plane**: `assigned_to` as a
  column (subjects incl. `team:`, filterable `"me"`/`"none"`, bulk assign) + an append-only
  comment thread, via `insight.assign`/`insight.comment` with their own member-grade caps.
  Decides the load-bearing dedup rule — a re-raise, including a re-open, never touches either —
  and that comments **do not evict** (the occurrence ring's shape, not its retention).
- [`insight-subscriptions-scope.md`](insight-subscriptions-scope.md) — subscribe a channel to
  all / one rule / one identity / a tag facet / a severity floor; matched at raise time.
- [`insight-notify-scope.md`](insight-notify-scope.md) — the anti-spam digest ladder
  (immediate → hourly → … → monthly), breakthroughs, ack-suppression, per-member kill switch.
- [`rule-raises-insight-scope.md`](rule-raises-insight-scope.md) — **the rule producer door**:
  a rule body raises (and **acks/closes**) an insight in one line via a new `insight` rhai
  handle over the existing `insight.raise`/`ack`/`resolve` verbs — no new verb, no new cap.
  Decides the `route:false` (read-only panel run) suppression and the emit/alert boundary.
</content>
