# Insights

Status: TODO — not shipped. The ask lives at
[`scope/insights/insights-scope.md`](../../scope/insights/insights-scope.md) (umbrella) plus
sub-scopes: [`insight-occurrences-scope.md`](../../scope/insights/insight-occurrences-scope.md)
(the per-insight transaction ring),
[`insight-subscriptions-scope.md`](../../scope/insights/insight-subscriptions-scope.md)
(channel subscriptions by rule/identity/tag-facet/severity), and
[`insight-notify-scope.md`](../../scope/insights/insight-notify-scope.md) (the adaptive
anti-spam digest ladder).

A durable, workspace-walled **data-insight record** (`insight:{ws}:{id}` — severity, origin
provenance, dedup/occurrence counting, `open → acked → resolved` lifecycle) raised from rules
(rhai handle), flows (`insight` sink node), or any principal via `insight.*` MCP verbs;
discovered through the tag graph; surfaced on an Insights page with the agent dock +
`builtin.insights-analyst` persona as the conversation layer.

Filled in when the feature ships (per `docs/ABOUT-DOCS.md`).
