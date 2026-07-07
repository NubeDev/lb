# Insights scope — a durable data-insight record over the shipped rules/flows/attention planes

Status: scope (the ask). Promotes to `public/insights/` once shipped.

An **insight** is a persisted, queryable data finding — "AHU-2 compressor short-cycling",
"card ending 4421 scored 0.93 fraud risk", "site-003 baseline energy up 18% week-on-week" —
raised by a rule, a flow, or an agent, carrying **severity**, **provenance** (what raised it,
from which run), **entity tags** (site/equip/point, account, …), and an
**open → acked → resolved** lifecycle with dedup/flap-suppression. Today a rule's `Finding`s
are ephemeral (returned from the run, gone after), and an inbox `Item` has no severity, no
dedup, no count, no structured ref — so nothing can answer *"show me all open critical faults
on site-A this week"*. Insights is that **one missing record type**; everything else in this
scope composes machinery that has already shipped.

## Is "insight" just a term? (the verdict)

Mostly yes — and the scope holds that line hard. What exists vs. what's missing:

| Need | Already shipped |
| --- | --- |
| Detect (compute over data) | `lb-rules` rhai cage + `Grid`/`Frame` stdlib, `source("series"\|"<datasource>"\|"query:<id>")` (`scope/rules/`) |
| Orchestrate / trigger | flows: `manual\|cron\|event\|inject\|boot` triggers, DAG, durable runs (`scope/flows/`) |
| Data in | ingest series + webhooks (`scope/ingest/`), `federation.query`/`mirror` (`scope/datasources/`) |
| Human attention | inbox `Item` + `Resolution` + approval reactor (`scope/inbox-outbox/`, `scope/rules/rules-approvals-scope.md`) |
| Must-deliver egress (email, external) | outbox `Effect` + `Target` + relay/backoff/dead-letter (`scope/inbox-outbox/outbox-scope.md`) |
| Conversation surface | channels (`channel.post`, any principal) + in-channel agent + the agent dock (`scope/channels/`, `scope/frontend/agent-dock-scope.md`) |
| Discovery / facets | tag graph — `tag:[key,value]` nodes, provenance edges, `tags.find` faceted intersection (`scope/tags/`) |
| AI analyst | agent + personas (`builtin.data-analyst` already has the whole data-verb menu) (`scope/agent-personas/`) |

This is the **umbrella**: it owns the record, the producer doors, and the page. Three
sub-scopes carry the key features and must be read by the implementing session:

- [`insight-occurrences-scope.md`](insight-occurrences-scope.md) — the per-insight
  **transaction log**: every raise appends one lite, size-capped occurrence row into a capped
  ring (last N), so "card 4421" keeps its recent transactions without becoming a time-series
  store.
- [`insight-subscriptions-scope.md`](insight-subscriptions-scope.md) — a member **subscribes a
  channel** to all insights, one rule (`origin_ref`), one identity (`dedup_key`), a **tag
  facet** (`siteRef:building-1`), or a severity floor; matched at raise time, delivered under
  the subscriber's stored principal.
- [`insight-notify-scope.md`](insight-notify-scope.md) — the **anti-spam digest ladder**: noisy
  keys decay immediate → hourly → daily → weekly → monthly summaries and climb back when
  quiet; breakthroughs (first occurrence, severity escalation, re-open) always deliver; ack
  suppresses; adjustable defaults + per-sub overrides + a per-member global kill switch.

The gap: **no durable record with severity + dedup + lifecycle + provenance**. `Finding` is a
per-run runtime value; `Item` deliberately stayed `{id, channel, author, body, ts}` (the
structured-`meta` extension was explicitly deferred); an outbox `Effect` is delivery motion,
terminal once delivered. This scope adds exactly that record and its verbs, and wires the three
producer doors and two consumer surfaces onto shipped seams. The two motivating verticals
(credit-card fraud, SkySpark-style HVAC/energy analytics) then need **zero core branches** —
they are datasources + rules + flows + this record, plus at most a domain extension.

## Goals

- One **generic, domain-free** `insight` record: severity, title/body, origin (rule/flow/agent
  ref + run), dedup key with occurrence counting, `open → acked → resolved` lifecycle.
- Three producer doors, all shipped seams: a **rhai handle** in the rules cage
  (`insight.raise(#{…})`, the rules-messaging pattern), a **built-in flow sink node**
  (`insight`), and the plain **MCP verb** for agents/extensions/manual.
- Faceted discovery through the **tag graph** (an insight is a taggable entity like anything
  else) — `siteRef`/`equipRef`/fault-kind facets give the SkySpark "spark list" for free.
- An **Insights page** (list/facet/detail/ack/resolve) whose AI story is the **shipped agent
  dock + a persona**, not new agent plumbing.
- Both use cases demonstrably buildable as config + (optionally) an extension: fraud =
  webhook → flow → rule → insight + outbox email; HVAC = timescale datasource → cron flow →
  rules over Haystack-tagged points (`docker/postgres/seed.py`) → tagged insights.

## Non-goals

- **On-call / paging / escalation-to-humans chains** (rotations, ack-or-reroute). Consumer-side
  routing IS in scope — but as the subscriptions + notify sub-scopes (a member subscribes a
  channel; the digest ladder tames the volume), not as a producer-side policy engine. Producers
  still author explicit `channel`/`outbox` sinks when the *flow itself* owns delivery.
- **Per-rule channels** (see rejected alternatives — this is the answer to "should a rule
  create a channel?": no).
- **ML anomaly detection in core.** Detection is rule/flow/agent authorship; a model-backed
  detector is an extension or `ai.*` inside a rule.
- **Retention UI.** The table is append-heavy; a purge/archival admin batch job mirrors
  `scope/jobs/job-retention-scope.md` as a follow-up, not this scope.
- **Insight rollups** (insights-about-insights, incident grouping). Later.
- **A dashboard insights widget.** Named follow-up under the widget platform
  (`scope/widgets/widget-platform-scope.md`); the page ships first.

## Intent / approach

A small **`lb-insights` crate** at the `lb-inbox` altitude (record + pure verbs over the store
seam, one verb per file) + host `insight.*` MCP tools + the rhai handle + the flow sink node +
`ui/src/features/insights/` + one data-only persona addition. State in SurrealDB; a
fire-and-forget bus event on raise for live UI; must-deliver delivery stays the outbox's job.

### Rejected alternatives (the design questions, answered)

1. **A channel per rule/insight — rejected.** Channels are cheap to create (a registry row,
   even implicit on first post), so the objection is not cost — it's that a channel is a
   *conversation surface*, not a store: flat ordered `Item` history, no severity, no facets, no
   lifecycle, no cross-channel query. 100 rules → 100 channels is attention fragmentation with
   no way to ask "what's open and critical?". Channels stay in the picture **explicitly**: a
   flow can add a `channel` sink posting a summary (+ deep link) into a team channel the humans
   chose (`#fraud-alerts`), and the conversation about an insight happens in the agent dock or
   that channel. Config, not machinery.
2. **Insight-as-outbox-effect (auto-approved effect to a channel) — rejected.** State vs
   motion (README §3): an outbox `Effect` is a fire-once *delivery intent* — opaque payload,
   terminal after `Delivered`, unqueryable as a record of fact, and `channel.post` doesn't need
   must-deliver anyway (it's an in-process host verb). An insight may *cause* an effect (send
   an email); it isn't one.
3. **Insight-as-inbox-`Item` — rejected.** `Item` has no severity/dedup/count/ref, and the
   inbox scope deliberately deferred a structured `meta` field to keep the shape stable.
   Overloading `body`-prefix tags (the `needs:approval` wart) for a whole severity+lifecycle
   grammar would compound the wart. The inbox remains the **attention bridge**: an insight that
   needs a human decision raises an approval item exactly as rules-approvals shipped it.
4. **Zero-new-code (samples + tags + inbox as "insights") — rejected.** Ingest samples are
   immutable time-points — no ack/resolve, no dedup/flap-suppression, no count. The lifecycle
   *is* the feature; it needs a mutable record.
5. **Insights as an extension — rejected for the primitive, embraced for the verticals.** The
   rules cage exposes only curated handles (inbox/outbox/channel — rules-messaging), so a rule
   can't reach an extension verb; and the record is as domain-free as inbox/outbox, which are
   core for the same reason. The **fraud system** and the **HVAC analytics app** are
   extensions/config on top (rule 10 intact: core never names them).

## The record

```
insight:{ws}:{id}
{
  id,                       // ulid
  dedup_key,                // caller-supplied stable identity, e.g. "rule:hunting:ahu-2"
  severity,                 // "info" | "warning" | "critical"  (closed v1 set)
  title,                    // one line, human
  body,                     // opaque JSON detail (evidence rows, scores, links)
  origin: { kind,           // "rule" | "flow" | "agent" | "ext" | "manual"
            ref,            // rule id / flow id / agent def / ext tool — opaque string
            run? },         // flow_run / job id when applicable — "where it was triggered from"
  status,                   // "open" | "acked" | "resolved"
  status_by?, status_ts?,   // who moved it last, when (logical ts)
  count, first_ts, last_ts, // LIFETIME occurrence accounting (monotonic)
  producer                  // host-stamped raising principal (un-spoofable, ingest pattern)
}
```

**Occurrences:** each raise also appends one lite row into a per-insight **capped ring** (last
N firings with their per-firing delta — score, reading, txn ref); `count` is the lifetime total
and may exceed the stored rows. Full contract in
[`insight-occurrences-scope.md`](insight-occurrences-scope.md).

**Dedup / flap suppression:** `insight.raise` is idempotent on `(ws, dedup_key)`. Existing
`open`/`acked` → bump `count` + `last_ts` (status untouched — an acked fault re-firing doesn't
re-page anyone). Existing `resolved` → **re-open** (status back to `open`, count continues) —
the fault came back. No matching key → create.

**Tags:** entity references ride the shipped tag graph, not columns —
`insight -> tagged -> tag:[siteRef, "site-003"]` with `Source::Producer` provenance, applied by
the raise verb from a `tags: {k: v}` argument. That buys `tags.find` faceted intersection and
the data-console for free. **Cardinality rule** (the 10k tag-node cap): tag values must be
low-cardinality dimensions (site/equip/fault-kind/rule-name — building-scale is fine);
per-transaction/card identities go in `dedup_key` and `body`, never tags.

## How it fits the core

- **Tenancy / isolation:** records keyed `insight:{ws}:{id}` in the workspace namespace;
  ws-B physically cannot list/get/ack ws-A insights; the watch subject is workspace-scoped.
- **Capabilities:** per-verb — `mcp:insight.raise:call` (producer-grade),
  `mcp:insight.list|get|watch:call` (read), `mcp:insight.ack|resolve:call` (member act). Deny
  is opaque. The rhai handle runs under `caller ∩ grant` and is charged to the run's
  `WriteMeter`, exactly like `inbox.record`/`channel.post` (rules-messaging).
- **Placement:** either. No reactor, no owner election — raise/ack/resolve are plain verbs;
  the only motion is the fire-and-forget UI event.
- **MCP surface (API shape, SCOPE-WRITTING §6.1):**
  - **Write:** `insight.raise { dedup_key, severity, title, body?, origin, tags?,
    occurrence? }` →
    `{ id, status, count, created }`; `insight.ack { id }`; `insight.resolve { id, note? }`.
    **No `update`/`delete` in v1** — it's an operational record; correction = resolve + raise;
    purge is the retention follow-up's admin batch job.
  - **Read:** `insight.get { id }`; `insight.list { status?, severity?, origin_ref?, tags?,
    range?, cursor?, limit? }` — keyset cursor per `scope/datasources/page-cursor-scope.md`.
  - **Live feed:** `insight.watch` → gateway SSE over bus subject `ws/{ws}/insight/events`
    (raise/ack/resolve events). Fire-and-forget — a durable consumer scans the table.
  - **Batch:** N/A — raise is single and bounded; bulk detection is a flow/rule loop whose
    durability is the flow run.
- **Data (SurrealDB):** one `insight` table (state) + tag edges. No new store.
- **Bus (Zenoh):** one fire-and-forget event subject for live UI. Must-deliver external
  notification is an **explicit outbox effect** authored by the producer — never raised
  implicitly by this crate.
- **Sync / authority:** ordinary workspace data; nothing insight-specific.
- **Secrets:** none.
- **SDK/WIT impact:** none — extensions raise insights via the existing host-callback MCP path
  under `caller ∩ install-grant`; no ABI change.
- **Skill doc:** YES — `skills/insights/SKILL.md` (raise/list/ack/resolve walkthrough grounded
  in a live run), written by the implementing session; plus a `core.insights` grounding skill
  for the persona (the core-skills seed pattern).

## Producers — the three doors

1. **Rules:** `insight.raise(#{ dedup_key, severity, title, body?, tags? })` registered in the
   one rhai cage beside the `inbox`/`outbox`/`channel` handles (rules-messaging pattern:
   explicit, caller-gated, write-metered). Host fills `origin = { kind:"rule", ref: rule_id,
   run: run_id }`. `emit()`/`alert()` stay unchanged — `emit` is per-run findings, `alert` is
   attention sugar; whether `alert` *also* writes an insight is an open question (recommend no
   in v1: keep raising explicit).
2. **Flows:** a built-in **`insight` sink node** (descriptor in the built-in pack; config
   schema = `severity`, `title` template, `dedup_key` template, `tags` map — templated over the
   `{payload, topic}` envelope like the `template` node). Host fills
   `origin = { kind:"flow", ref: flow_id, run: run_id }`.
3. **Everything else:** the plain MCP verb — an agent under its derived principal, an extension
   via host-callback, a human via the page/CLI (`origin.kind = "agent" | "ext" | "manual"`).

## Consumers

- **Insights page** — `ui/src/features/insights/`: faceted list (status / severity / tags /
  time, keyset-paged), detail drawer (body evidence, origin deep link to the rule/flow/run,
  occurrence history), ack/resolve actions, deep-linkable route (routing scope). Live via
  `insight.watch` SSE.
- **The AI story — no new agent surface.** The shipped **agent dock** already rides every page
  with page context injected; add **`builtin.insights-analyst`** to `personas.toml` —
  `extends: ["builtin.data-analyst"]` + `insight.*` verbs + the `core.insights` grounding
  skill. Pure data (the persona swap test: zero code). A user on the Insights page opens the
  dock and asks "why is AHU-2 hunting?" — the persona answers via `insight.get` →
  `series.read`/`federation.query` → `rules.get`, under `persona ∩ agent ∩ caller`. Per-page
  auto-persona pinning stays the personas scope's deferred question; until then the dock's
  explicit persona pick suffices.
- **Channels — two doors, neither per-rule:** producer-side, a `channel` sink beside the
  `insight` sink posts a summary + link into a team-chosen channel; consumer-side, a member
  **subscribes** a channel to a rule / identity / tag facet / severity floor
  ([`insight-subscriptions-scope.md`](insight-subscriptions-scope.md)), with all deliveries
  tamed by the digest ladder ([`insight-notify-scope.md`](insight-notify-scope.md)). One
  channel per *team concern*, never per rule.
- **Approvals:** an insight needing a human decision (fraud: "block this card?") composes the
  shipped loop — the same rule calls `inbox.request_approval` staging a `held` outbox effect;
  the insight records the fact, the approval gates the action.

## Example flows

**Credit-card fraud (webhook-fed, extension-delivered):**
1. Admin creates a `signature`-mode webhook (shipped); the processor POSTs transactions to
   `POST /hooks/{ws}/{id}` → samples on `webhook:{ws}:{id}`.
2. A flow's `event` trigger watches that series; a `rhai` node scores the transaction
   (stdlib stats / `ai.classify`).
3. Score ≥ threshold → the **`insight` sink** raises
   `{ dedup_key: "fraud:"+card_ref, severity: "critical", tags: { kind: "fraud" } }` →
   the **`channel` sink** posts to `#fraud-alerts` → an **`outbox` effect**
   (`target:"email", action:"notify"`) for must-deliver mail. (The email `Target` doesn't
   exist yet — the outbox scope's static-target-set gap; it's the named prerequisite for the
   mail leg, or v1 ships channel-only delivery.)
4. Repeat hits on the same card bump `count`, not the channel. An analyst acks, investigates
   with the dock persona, resolves.

**HVAC / energy analytics (the SkySpark shape, over `docker/postgres/seed.py`):**
1. `datasource.add` the seeded TimescaleDB; rules read it live via `source("timescale")` or
   `federation.mirror` caches ranges into series.
2. A `cron` flow (nightly + 15-min) runs saved rules over the Haystack-tagged points —
   short-cycling, setpoint hunting, schedule violation, baseline drift (the seed's clean
   physics makes injected faults testable).
3. Each finding raises an insight tagged `{ siteRef, equipRef, kind: "short-cycle" }` with
   `dedup_key: rule+equip` — recurring faults accumulate `count`, not rows.
4. The Insights page faceted by site/equip/kind **is** the spark list; the dock persona answers
   "which sites got worse this month?" via `insight.list` + `federation.query`.

Both verticals: zero core branches; core never learns the words "fraud" or "HVAC".

## Testing plan

Real store/bus/gateway, seeded records, no fakes (rule 9). Mandatory + key cases:

- **Capability deny (mandatory):** each `insight.*` verb denied without its cap; a rule whose
  caller lacks `mcp:insight.raise:call` gets a cage deny (and the run continues per
  rules-messaging error semantics); ack/resolve denied to a read-only principal.
- **Workspace isolation (mandatory):** ws-B `list`/`get`/`ack` cannot see or touch ws-A
  insights; the watch subject leaks nothing cross-ws; tag facets stay ws-scoped.
- **Dedup lifecycle:** raise → raise same key (count=2, still one row, status preserved when
  `acked`) → resolve → raise again (re-opened, count=3). Concurrent same-key raises settle to
  one row.
- **Producer doors:** a saved rule raising via the handle (write-metered, origin stamped
  `rule`); a flow run through the `insight` sink (origin carries `flow` + run id); plain verb.
- **Discovery:** `insight.list` by status/severity + tag facet returns exactly the seeded
  matches; keyset paging over >1 page.
- **Live feed:** raise/ack/resolve each produce one SSE event on the ws subject.
- **Persona:** `builtin.insights-analyst` seeds; menu = persona ∩ caller (a caller without
  `insight.list` still denied through the persona).
- **UI:** page gateway test (list/facet/ack against a spawned `test_gateway`), plus the two
  mandatory categories at the UI layer.

## Risks & hard problems

- **Tag cardinality.** The 10k tag-node cap makes high-cardinality tag values (txn ids, card
  numbers) a real foot-gun; the raise verb should reject obviously-unbounded values only by
  documentation + the existing cap deny — producers must be taught (skill doc) that identity
  lives in `dedup_key`.
- **Insight storms.** Same-key storms are absorbed by dedup; a misfiring rule minting *new*
  keys per tick is bounded only by the rules `WriteMeter` / flow-run scope. A per-producer
  raise budget is an open question, not v1.
- **Unbounded growth.** Append-heavy table + "no delete" = the job-retention problem again;
  the purge/archival admin batch job follow-up must land before any production fleet.
- **Delivery expectations.** An insight with no matching subscription and no producer-authored
  sink reaches nobody; the page must make "0 subscribers" visible on an insight, or resolved
  insights that never reached a human become a trust bug.
- **Outbox target gap.** Email/SMS delivery needs a `Target` the static set doesn't have —
  inherited from the outbox scope, surfaced here because fraud is the first real demand.

## Open questions

> **v1 dispositions (shipped 2026-07-05):** the record + occurrences + subscriptions + notify ladder
> all shipped (MCP verbs, gateway REST+SSE, UI, persona, skill). What's marked "Resolved v1" below is
> closed; the rest are documented follow-ups, not gaps. The two producer doors (rhai handle + flow
> sink node) are the named deferred slice — today producers reach `insight.raise` via the MCP verb.

1. Should `alert(...)` in the rules cage also raise an insight (one call = finding + attention
   + record), or stay attention-only? **Open follow-up** (recommend stay; revisit after real rule
   authorship). Not a v1 gap — the explicit `insight.raise` MCP verb is the door today.
2. Severity: closed `info|warning|critical` forever, or admin-extensible? **Resolved v1: closed**
   — extra dimensions ride tags; reopen when a vertical proves a fourth level.
3. Per-producer raise quota (new-key rate limit) — needed, or is WriteMeter + flow bounds enough?
   **Open follow-up** — same-key storms are absorbed by dedup; new-key storms are bounded by the
   rules `WriteMeter` / flow-run scope today.
4. Retention default for `resolved` insights (90 days? admin-set?) — decided in the retention
   follow-up. **Open follow-up** (the `job-retention-scope.md` precedent) — must land before any
   production fleet (append-heavy tables).
5. Auto-pinning the analyst persona when the dock opens on the Insights page — personas catalog
   Q3; mechanism exists (per-invoke override), only the mapping is deferred. **Open follow-up**
   (personas-catalog Q3).
6. Sub-scope questions live with their docs: occurrence ring per-insight sizing (occurrences Q1 —
   resolved v1 workspace-only), sub filter globs / team-owned subs / inbox sink (subscriptions Q1–3 —
   resolved exact-only / open / open), severity-tiered cooldowns / quiet hours / AI-narrated digests
   (notify Q1–4 — resolved one-cooldown / open / resolved deliveries-worth / open).

## Related

- Sub-scopes: [`insight-occurrences-scope.md`](insight-occurrences-scope.md),
  [`insight-subscriptions-scope.md`](insight-subscriptions-scope.md),
  [`insight-notify-scope.md`](insight-notify-scope.md)
- `scope/rules/rules-messaging-scope.md` (the handle pattern), `rules-approvals-scope.md`
  (the human-decision loop), `data-stdlib-scope.md` (detection compute)
- `scope/flows/flows-scope.md`, `data-nodes-scope.md` (sink-node + template precedent),
  `triggers-lifecycle-scope.md`
- `scope/ingest/ingest-scope.md`, `webhooks-scope.md` (inbound data), `scope/datasources/`
  (federation/mirror, `page-cursor-scope.md`)
- `scope/inbox-outbox/inbox-outbox-scope.md`, `outbox-scope.md` (attention vs delivery — the
  planes this record deliberately is not)
- `scope/tags/tags-scope.md` (facets + provenance edges + the cardinality cap)
- `scope/agent-personas/persona-model-scope.md`, `persona-catalog-scope.md`
  (`builtin.data-analyst`, the extends/seed pattern), `scope/frontend/agent-dock-scope.md`
- `scope/jobs/job-retention-scope.md` (the growth problem's precedent)
- `docker/postgres/seed.py` (the HVAC/energy worked-example substrate)
- On ship: `public/insights/insights.md`, `skills/insights/SKILL.md`, `core.insights` skill
