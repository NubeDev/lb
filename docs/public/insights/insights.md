# Insights (shipped)

Status: **shipped (S8)**. A durable, workspace-walled **data-insight record** with severity,
origin provenance, dedup/occurrence counting, and an `open → acked → resolved` lifecycle — raised
by any principal via the `insight.*` MCP verbs, discovered through the tag graph, pushed into
channels via subscriptions tamed by an adaptive digest ladder, and surfaced on an Insights page
with the agent dock + `builtin.insights-analyst` persona as the conversation layer.

- The ask (source of truth): [`scope/insights/insights-scope.md`](../../scope/insights/insights-scope.md)
  (umbrella) + [`insight-occurrences-scope.md`](../../scope/insights/insight-occurrences-scope.md)
  + [`insight-subscriptions-scope.md`](../../scope/insights/insight-subscriptions-scope.md) +
  [`insight-notify-scope.md`](../../scope/insights/insight-notify-scope.md).
- How it was built: [`sessions/insights/insights-session.md`](../../sessions/insights/insights-session.md)
  (finishing) + [`insights-scaffold-session.md`](../../sessions/insights/insights-scaffold-session.md).
- How to drive it: [`skills/insights/SKILL.md`](../../skills/insights/SKILL.md).
- Working history: `debugging/insights/` (five entries: envelope-unwrap, oseq-collision,
  prefs-schema-column, core-skills-assertion, ui-bare-rounded).

## What shipped

| Layer | Where | What |
|---|---|---|
| Record + pure verbs | `rust/crates/insights/` (`lb-insights`) | `raise` (dedup/re-open), `get`, `list` (faceted, keyset-paged), `ack`, `resolve`, `occurrences` (the ring), `sub.*`, `policy.*`, `match_subs`, `ladder_step`, `compute_due_digests`, `apply_intents`. One verb per file; no auth here. |
| Host service | `rust/crates/host/src/insight/` | Capability-gated wrappers + the `call_insight_tool` MCP bridge; host-stamps `producer`/`owner`/`acked_by`; `insight_raise` threads `&Arc<Node>` (tags, bus event, matcher→apply_intents→deliver_to_sub); the digest reactor + the SSE watch subscription. |
| Gateway | `rust/role/gateway/src/routes/insight.rs` | `GET /insights`, `GET /insights/{id}`, `POST /insights/{id}/{ack,resolve}`, `GET /insights/{id}/occurrences`, `GET /insights/events` (SSE). |
| UI | `ui/src/features/insights/` + `ui/src/lib/insights/` | Page, list, detail drawer, facets sidebar, actions; `useInsights` (paging/SSE/act), `insights.api.ts` (mcp_call bridge), `insights.events.ts` (EventSource). |
| Persona | `rust/crates/host/src/agent/personas/personas.toml` | `builtin.insights-analyst` (extends `data-analyst`; investigate verbs only; no `raise`; grounding `core.insights`). |
| Skill | `docs/skills/insights/SKILL.md` | Embedded at boot as `core.insights`; the persona's grounding skill + the operator walkthrough. |
| Prefs | `rust/crates/prefs/` | `insight_notifications: Option<bool>` (the per-member kill switch), in `Prefs` + both SCHEMAFULL tables + the column projection. |

### The record

```
insight:{ws}:{id}
{ id, dedup_key, severity, title, body, origin:{kind,ref,run?},
  status, status_by?, status_ts?, count, first_ts, last_ts, producer }
```

- `severity`: `info | warning | critical` (closed v1; extra dimensions are tags).
- `origin.kind`: `rule | flow | agent | ext | manual` (host-forced from the door you called through).
- `status`: `open → acked → resolved`. Resolved + a new raise ⇒ **re-open** (count continues).
- `dedup_key`: the stable identity (e.g. `"fraud:card-4421"`, `"rule:hunting:ahu-2"`). Identity
  lives here / in `body`, **never in tags** (tags are low-cardinality dimensions — site/equip/kind).

### Occurrences — the per-insight transaction ring

Every raise appends one lite row into a capped ring under the insight (default 100, admin-adjustable
`[0, 1000]` via the policy `ring_cap`). The parent's `count` is the lifetime truth; the ring is the
recent evidence (may be fewer rows). `occurrence.data` is hard-capped at **2 KB** serialized —
oversize rejects the whole raise (never silent truncation, never an orphan row). Wire field is
**`oseq`** (the per-insight monotone number = the parent's lifetime count at append).

### Subscriptions — push into a channel

A member subscribes a channel to all insights, one rule (`origin_ref`), one identity (`dedup_key`),
a tag facet (`{siteRef: "building-1"}`), or a severity floor. Filter axes AND-compose; delivery
happens under the subscriber's stored principal, re-checked at fire time (deny ⇒ dormant + owner
inbox note, never silent). Hard cap 1,000 subs per workspace.

### The digest ladder (anti-spam) + the kill switch

Per `(subscription, dedup_key)` delivery levels `L0 immediate → L1 hourly → L2 daily → L3 weekly →
L4 monthly`. **Breakthroughs** (first-key, severity escalation, re-open) always deliver. **Ack
suppresses.** Escalate on ≥3 deliveries-worth of noise in a window; decay one level per fully-quiet
window. **Digests are one message per `(sub, window)`** (idempotent). Per-sub `throttle_override`
pins a level; per-sub `muted`; **per-member kill switch** (`prefs.insight_notifications = false`).
A durable reactor scans on the injected clock; exactly one node drives a workspace's digests
(owner-election precedent).

### The AI analyst — no new agent surface

The shipped agent dock rides the Insights page with page context injected; `builtin.insights-analyst`
carries the investigation verbs and is grounded by `core.insights`. A user asks "why is AHU-2
hunting?" — the persona answers via `insight.get` → `series.read`/`federation.query` → `rules.get`,
under `persona ∩ agent ∩ caller`.

## Tests (green)

Real store/bus/gateway/caps, seeded through the real write path (rule 9 — no fakes, no `*.fake.ts`).

- **`lb-insights` ladder unit (pure state machine):** `cargo test -p lb-insights --test ladder_test`
  — 10/10.
- **Host integration over a real `Node`:** `cargo test -p lb-host --test insights_test` — 14/14
  (per-verb cap-deny, ws-isolation on list + occurrences, dedup-lifecycle, ring-cap, 2 KB-reject,
  matcher tag-axis, ladder cooldown, digest idempotency, kill switch).
- **Gateway REST + MCP bridge:** `cargo test -p lb-role-gateway --test insight_routes_test` — 4/4
  (per-route cap-deny, ws-isolation, the raise→list→get→ack→resolve round-trip).
- **UI against a real spawned gateway:** `pnpm test:gateway src/features/insights/InsightsPage.gateway.test.tsx`
  — 4/4.
- **Core skills seed:** `core.insights` embeds a non-empty body and is in `DEFAULT_CORE_SKILLS`
  (`cargo test -p lb-host --test core_skills_test` — 11/11).

## Known follow-ups (recorded, not silently dropped)

- **InsightDetail origin deep-link** — the drawer shows the origin but the click-through to the
  rule/flow/run route is NOT wired (the workspace id isn't threaded into the drawer). The body
  evidence renderer is also still a JSON dump (typed table/chart renderer is the dashboard-widget
  precedent follow-up).
- **Producer doors (umbrella §"Producers")** — the rhai cage `insight.raise(#{…})` handle and the
  built-in `insight` flow sink node are deferred. Today producers reach `insight.raise` via the MCP
  verb (agents/extensions/CLI/manual).
- **Retention/purge** — the append-heavy `insight`/`insight_occ`/`insight_notify` tables need the
  retention follow-up (the `scope/jobs/job-retention-scope.md` precedent) before any production
  fleet.
- **Email/webhook digest delivery** — channel-only v1; the outbox sink kind arrives with the email
  `Target` (umbrella scope's named gap).

## How it fits the core

- **Symmetric nodes (rule 1):** one binary, role by config — no `if cloud` branch; the digest
  reactor follows the flows/reminders owner-election precedent on every node.
- **One datastore (rule 2):** SurrealDB only — `insight`, `insight_occ`, `insight_sub`,
  `insight_notify`, `insight_policy` tables; the ring rides `lb_store::capped`.
- **State vs motion (rule 3):** the records + ladder state are state; deliveries are
  `channel.post` (durable Item + bus motion). The raise bus event is fire-and-forget (live UI only).
- **Capability-first (rule 5):** per-verb `mcp:insight.<verb>:call`; `sub.create` double-gated
  (verb cap + `bus:chan/{channel}:pub`, re-checked at fire).
- **Workspace wall (rule 6):** every record keyed `insight:{ws}:{id}`; ws-B physically cannot
  list/get/ack ws-A insights; the watch subject is ws-scoped.
- **MCP contract (rule 7):** every verb (raise/get/list/ack/resolve/occurrences/sub.*/policy.*)
  rides `POST /mcp/call`; the REST routes are the page's convenience surface over the same verbs.
- **Core knows no extension (rule 10):** domain-free — core never learns "fraud" or "HVAC"; those
  verticals are datasources + rules + flows + tags on top.
