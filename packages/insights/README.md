# @nube/insights

The Lazybones **"triage findings raised by rules, flows, and agents"** machinery, extracted from the
dashboard so any surface reuses ONE implementation — the Insights page, a **dashboard widget**
(read-only or acknowledge), a standalone extension UI.

It is **transport-agnostic**: the host injects an `InsightsClient` (how to reach the node's
`insight.*` verbs). The shell delegates to its `/mcp/call` bridge + SSE hub; an extension delegates to
its host bridge; a dashboard widget shares the shell's. The package never imports an API client,
`invoke`, or `@/` — that's what makes one implementation work everywhere.

The **look is optional**: bring your own skin and drive only the headless hooks + model helpers, or use
the batteries-included widgets (self-themed via scoped `--ins-*` tokens).

## Three layers — adopt what you need

```ts
import {
  // MODEL (pure): tone keys, ordering, formatters — for a host that renders its own look
  severityTone, statusTone, timeAgo, originLine,
  // HOOKS: data + state over the injected client
  useInsights, useInsight,
  // UI (optional look): the dashboard widgets + composable primitives
  InsightsReadWidget, InsightsAckWidget, InsightsWidget,
  InsightRow, InsightActions, SeverityBadge, StatusBadge,
  type InsightsClient, type Insight,
} from "@nube/insights";
import "@nube/insights/style.css"; // only if you use the package's own look
```

## Wiring (the injected seam)

```ts
// The host implements the reads/acts over its own transport. `subscribe` is optional.
const client: InsightsClient = {
  list: (q) => listInsights(q),          // insight.list
  get: (id) => getInsight(id),           // insight.get
  ack: (id) => ackInsight(id),           // insight.ack   (host stamps ts)
  resolve: (id, note) => resolve(id, note), // insight.resolve
  occurrences: (id, c, l) => occ(id, c, l), // insight.occurrences
  subscribe: (onEvent) => subscribe(onEvent), // optional live tail → head refresh
};
```

Every read may reject (a denied/absent cap) — the hooks surface that as an honest **error**, never a
fabricated list (CLAUDE §9). The host re-checks `mcp:insight.<verb>:call` + the workspace wall on every
call regardless of the UI gate.

## The two dashboard widgets

```tsx
// Read-only — a glanceable list, no action buttons.
<InsightsReadWidget client={client} filter={{ severity: "warning", limit: 10 }} />

// Acknowledge — each open/acked row gets ack / resolve / dismiss inline.
<InsightsAckWidget client={client} filter={{ status: "open" }} />
```

Both are `InsightsWidget` with `interactive` pinned. `onSelect` makes rows clickable (open a host detail
surface); `paged`, `showRefresh`, `title` tune the frame. **Dismiss** is a *local hide* (this widget
only), distinct from **resolve** (a durable status change).

## Bring your own look

```tsx
function MyList({ client }: { client: InsightsClient }) {
  const { items, act } = useInsights(client, { limit: 20 });
  return items.map((it) => (
    <MyRow key={it.id} tone={severityTone(it.severity)} when={timeAgo(it.last_ts)}
           onAck={() => act(it.id, "ack")} insight={it} />
  ));
}
```

## Reference client

`memoryClient(seed)` is a **real** in-memory `InsightsClient` over an array of records (ack/resolve
mutate them) — for host demos and tests. `denyClient()` models the capability-deny path. Neither is a
`*.fake.ts`: the client IS the boundary the package is defined against.

## Theming

Self-themed via `--ins-*` tokens scoped to `.ins-root`, aliasing the host's shadcn vars (`--bg`,
`--fg`, `--border`, `--accent`, `--destructive`, `--warning`, `--success`) with dark fallbacks. Override
by setting `--ins-*` on any ancestor. No preflight, no global utilities — the stylesheet can't touch the
host app.

Scope: `docs/scope/insights/insights-package-scope.md`.
