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
- [`insight-subscriptions-scope.md`](insight-subscriptions-scope.md) — subscribe a channel to
  all / one rule / one identity / a tag facet / a severity floor; matched at raise time.
- [`insight-notify-scope.md`](insight-notify-scope.md) — the anti-spam digest ladder
  (immediate → hourly → … → monthly), breakthroughs, ack-suppression, per-member kill switch.
- [`rule-raises-insight-scope.md`](rule-raises-insight-scope.md) — **the rule producer door**:
  a rule body raises (and **acks/closes**) an insight in one line via a new `insight` rhai
  handle over the existing `insight.raise`/`ack`/`resolve` verbs — no new verb, no new cap.
  Decides the `route:false` (read-only panel run) suppression and the emit/alert boundary.
</content>
