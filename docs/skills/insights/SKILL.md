---
name: insights
description: >-
  Raise, list, investigate, ack, and resolve durable data-insight records over the `insight.*` MCP
  verbs and the `/insights` REST + SSE surface. An insight is a persisted, queryable data finding
  ("AHU-2 short-cycling", "card ending 4421 scored 0.93 fraud risk") raised by a rule, a flow, or
  any principal — severity, origin provenance, dedup-keyed occurrence counting, and an
  `open → acked → resolved` lifecycle. Subscribe a channel to all insights, one rule, one identity,
  a tag facet, or a severity floor; an adaptive digest ladder tames the volume (immediate → hourly →
  daily → weekly → monthly), breakthroughs always deliver, ack suppresses, and a per-member kill
  switch disables the lot. Use when a task says "raise an insight", "list open critical insights",
  "ack/resolve a finding", "subscribe a channel to a tag facet", "tune the digest ladder", or "why
  did/didn't this insight notify anyone". Domain-free: core never learns "fraud" or "HVAC" — those
  are datasources + rules + flows + tags on top of this record.
---

# Insights (`insight.*` + `/insights`) — the durable data-finding record

An **insight** is one missing record type: a persisted, queryable data finding with **severity**,
**provenance** (what raised it, from which run), **entity tags**, and an
`open → acked → resolved` lifecycle with dedup/flap-suppression. A rule's `Finding` is ephemeral
(gone after the run); an inbox `Item` has no severity/dedup/count. Insights fills that gap — and
everything else (rules, flows, channels, the tag graph, the agent dock) composes onto it.

- The record + pure verbs: `rust/crates/insights/` (`lb-insights`, the `lb-inbox` altitude — one
  verb per file, no auth here).
- The capability-gated host service: `rust/crates/host/src/insight/` (one verb per file) —
  authorizes first, host-stamps `producer`/`owner`/`acked_by` (un-spoofable), then delegates.
- The record: `insight:{ws}:{id}` → `{id, dedup_key, severity, title, body, origin, status,
  status_by?, status_ts?, count, first_ts, last_ts, producer}`.

**`insight.*` is reached two ways:** the universal MCP bridge (`POST /mcp/call` — every verb,
used by agents/extensions/CLI) AND dedicated REST routes (`/insights…` — the page's read/act
surface). Workspace + principal come from the token (the hard wall); each verb authorizes first,
denials are opaque.

## 1. Authenticate

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

Capabilities — one per verb: `mcp:insight.raise:call` (producer-grade write),
`mcp:insight.list|get|watch:call` (read), `mcp:insight.ack|resolve:call` (member act),
`mcp:insight.occurrences:call` (evidence — a stronger read than the headline),
`mcp:insight.sub.<create|list|get|delete|mute>:call` (channel subscriptions),
`mcp:insight.policy.<get|set>:call` (admin). `insight.sub.create` **also** requires the caller hold
`bus:chan/{channel}:pub` at create time (no-widening up front), re-checked at fire time.

## 2. The verbs

| Verb | Args | Result |
|---|---|---|
| `insight.raise` | `dedup_key, severity, title, body?, origin, tags?, occurrence?, evidence?, analysis?, ts` | `{id, status, count, created, reopened, dedup_key, severity, kind}` (idempotent on `(ws, dedup_key)`) |
| `insight.get` | `id` | the full record (incl. `evidence`, `analysis`, the `tags` echo, + the full `comments` thread) |
| `insight.list` | `status?, severity?, origin_ref?, tags?, range?, assigned_to?, cursor?, limit?` | `{items:[Insight], next?}` (newest-first, keyset-paged; rows carry the `tags` echo + `assigned_to`, **not** `evidence`/`analysis`/`comments`) |
| `insight.ack` | `id, ts` | `{ok:true}` (`open → acked`) |
| `insight.resolve` | `id, note?, ts` | `{ok:true}` (`* → resolved`, idempotent) |
| `insight.assign` | `id` \| `ids[≤100]`, `assignee?` (`null` clears) | `{assigned_to}` for `id`; `{results:[{id,ok,error?}]}` for `ids` |
| `insight.comment` | `id, text, ts` | `{seq}` (append-only; author host-stamped) |
| `insight.occurrences` | `insight_id, cursor?, limit?` | `{items:[Occurrence], next?}` (newest-first ring) |
| `insight.sub.create` | `sink{kind,channel}, filter{…}, throttle_override?, now` | `{id}` |
| `insight.sub.list` | `all?` | `{subs:[Subscription]}` (own; admin `all=true` ⇒ workspace) |
| `insight.sub.get` / `.delete` / `.mute` | `id` (+`muted` for mute) | the sub / `{ok:true}` |
| `insight.policy.get` / `.set` | (`Policy` for set) | the workspace policy (defaults if no record) |
| `insight.watch` | — (SSE) | live raise/ack/resolve events on `ws/{ws}/insight/events` |

- **`severity`** is `"info" | "warning" | "critical"` (closed v1 set; extra dimensions are tags).
- **`origin`** is `{kind: "rule"|"flow"|"agent"|"ext"|"manual", ref, run?}`. The host forces
  `kind` from the door you called through (a rule's handle can't claim `kind:"manual"`); `ref`/`run`
  are opaque strings the deep-link surface reads.
- **`ts`** / **`now`** are caller-supplied logical timestamps (determinism, README §3) — pass a real
  monotone value. The gateway REST routes inject `gw.now()` so the browser passes none.
- **No `update`/`delete` in v1** — it's an operational record; correction = resolve + raise; purge
  is the retention follow-up's admin batch job. The triage verbs are deliberately **two narrow
  verbs, not one `update`**: each has its own cap, so a producer grant buys no triage write power.
- **`assigned_to` is a SUBJECT, not a user id** — `user:priya` *or* `team:mechanical` (queue
  ownership is legal from v1). Never assume a `user:` prefix when rendering or parsing.
- **`evidence`** is the finding's *data* binding (datasource + plottable series + threshold/window) —
  a descriptor the node never executes. **`analysis`** is the producer's *reasoning* (§3b below).
  Both are `get`-only and both **refresh on supply**, independently of each other.

### The six `analysis` fields (and the closed-struct rule)

`analysis` is the producer's own explanation, structured so every consumer renders the same labels in
the same order and an agent can name them:

| Field | What it says | Example |
|---|---|---|
| `trigger_logic` | why it fired, in the producer's words | `"Zero water consumption for 24 consecutive hours"` |
| `suspected_cause` | the **hypothesis** — never a diagnosis | `"Meter offline or site unoccupied (weekend)"` |
| `normalised_metric` | the metric judged | `"Daily water usage (kL)"` |
| `benchmark_context` | what it was compared against | `"vs expected minimum baseline"` |
| `deviation` | how far off — a `Quantity` | `{value: -100.0, unit: "%", note: "vs 1.8 kL baseline"}` |
| `estimated_impact` | consequence if unaddressed — a `Quantity` | `{value: 180.0, unit: "AUD/day"}` |

**⚠ The struct is CLOSED. A seventh key is accepted and SILENTLY DROPPED — put anything else in
`body`.** That is deliberate (a fixed vocabulary is the whole feature, and a free map would give every
producer a private one), but it means a field you invent will never error and never store. `body` is
the documented overflow.

A **`Quantity`** is `{value?, unit?, note?}` and exists so findings can be *ranked* ("show me today's
findings by cost"). Three legal shapes: note-only (`{note: "N/A (data quality)"}` — the honest "we
considered it and it doesn't apply", which a bare number would force you to drop), `value` + `unit`,
or all three. Two are **refused**: a `value` with no `unit` (an uninterpretable number that breaks the
cross-finding aggregation the type exists for) and an all-absent `{}` (which says less than omitting
the field). `unit` is free text — a consumer summing across producers must **group by `unit`** and
refuse to add unlike units, since nothing stops one rule writing `"AUD/day"` and another `"$/day"`.
`suspected_cause` is named to hedge; a UI must carry that hedge in the **label** too.

## 3. Raise → dedup → list → ack → resolve

```bash
BASE=http://127.0.0.1:8080/mcp/call
auth=(-H "authorization: Bearer $TOKEN" -H 'content-type: application/json')

# 1. raise — a fraud-styled critical finding (identity lives in dedup_key/body, NEVER the title)
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.raise","args":{
  "dedup_key":"fraud:card-4421",
  "severity":"critical",
  "title":"score above threshold",
  "body":{"score":0.93,"amount":412.50},
  "origin":{"kind":"rule","ref":"rule:scorer","run":"job:1"},
  "tags":{"kind":"fraud"},
  "occurrence":{"data":{"score":0.93,"txn":"t-88123"},"severity":"critical"},
  "ts":1719800000000}}'
# → {"id":"01H…","status":"open","count":1,"created":true,"reopened":false,
#    "dedup_key":"fraud:card-4421","severity":"critical","kind":"raise"}

# 2. same dedup_key again ⇒ bumps count + last_ts, status UNTOUCHED (no re-page)
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.raise","args":{
  "dedup_key":"fraud:card-4421","severity":"critical","title":"score above threshold",
  "origin":{"kind":"rule","ref":"rule:scorer"},"ts":1719800001000}}'
# → {"id":"01H…"(same),"count":2,"created":false,…}

# 2b. a raise carrying its own REASONING — the drawer renders these six labels; the roster never
#     sees them. `deviation`/`estimated_impact` are Quantities so a report can RANK by the number.
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.raise","args":{
  "dedup_key":"rule:no-water-1d:WM-CHU-01",
  "severity":"warning",
  "title":"Chullora — no water usage in 1 day",
  "origin":{"kind":"rule","ref":"rule:no-water-1d"},
  "tags":{"building":"chullora-dc","asset_type":"water-meter"},
  "analysis":{
    "trigger_logic":"Zero water consumption for 24 consecutive hours",
    "suspected_cause":"Meter offline or site unoccupied (weekend)",
    "normalised_metric":"Daily water usage (kL)",
    "benchmark_context":"vs expected minimum baseline",
    "deviation":{"value":-100.0,"unit":"%","note":"vs 1.8 kL baseline"},
    "estimated_impact":{"value":180.0,"unit":"AUD/day"}},
  "ts":1719800000000}}'
# → {"id":"01K…","count":1,"created":true,…}
# A re-raise SUPPLYING analysis overwrites it (so a rule that learns a real baseline heals the
# drawer); a re-raise OMITTING it leaves the stored reasoning alone. Refuses: a `value` with no
# `unit`, an empty `{}` quantity, and an analysis over 4 KB — each rejects the WHOLE raise before
# any write, so there is never an orphan record.

# 3. list — open critical insights, keyset-paged. Rows carry their TAG ECHO, so a roster renders
#    dimension columns from THIS call alone — no follow-up tags.find, no tags cap needed.
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.list","args":{
  "status":"open","severity":"critical","limit":50}}'
# → {"items":[{"id":"01H…","dedup_key":"fraud:card-4421","severity":"critical","status":"open",
#              "count":2,"last_ts":…,"tags":{"kind":"fraud"}},…],"next":…}

# 3b. filter BY a facet — resolved through the tag GRAPH (not the echo), so it is correct even for
#     a record whose echo hasn't caught up to an out-of-band tags.* change yet.
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.list","args":{
  "tags":{"kind":"fraud"},"status":"open","limit":50}}'

# 4. ack (open → acked) — "I know, investigating"
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.ack","args":{"id":"01H…","ts":1719800002000}}'

# 5. resolve — with an optional note
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.resolve","args":{
  "id":"01H…","note":"false positive — merchant verified","ts":1719800003000}}'

# 6. resolved + raise AGAIN ⇒ re-open (status→open, count continues, kind=reopen breakthrough)
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.raise","args":{
  "dedup_key":"fraud:card-4421","severity":"critical","title":"score above threshold",
  "origin":{"kind":"rule","ref":"rule:scorer"},"ts":1719800004000}}'
# → {"status":"open","count":3,"reopened":true,"kind":"reopen",…}
```

**REST equivalent** for the page's read/act surface:
- `GET /insights?status=open&severity=critical` — list (filter axes as query params).
- `GET /insights/{id}` — one record.
- `POST /insights/{id}/ack` / `POST /insights/{id}/resolve` (optional `{"note":"…"}` body) —
  `ts` injected from the gateway clock.
- `GET /insights/{id}/occurrences?cursor.seq=…&limit=50` — the per-firing ring.

## 3c. Triage — who owns it, and what we found out

The **human** plane beside the machine's record: one owner axis and an append-only note thread. Two
verbs, two caps (`mcp:insight.assign:call`, `mcp:insight.comment:call`) — a producer holding only
`insight.raise` gets **neither**, which is why there is no generic `insight.update`.

```bash
# The triage queue: what nobody owns yet.
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.list","args":{
  "status":"open","assigned_to":"none"}}'
# → rows carry `assigned_to` (absent = unassigned) — the owner column, no N+1

# 1. Take it. `assignee` is a SUBJECT: user: or team: (both legal from v1).
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.assign","args":{
  "id":"01H…","assignee":"user:priya"}}'
# → {"assigned_to":"user:priya"}
# Refused (opaquely) if the subject is not a member/team OF THIS WORKSPACE — the same error a
# subject that doesn't exist gets, so a probe can't confirm someone exists in another tenant.

# 2. Say what you found. `author` is host-stamped from your token — supplying one is ignored.
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.comment","args":{
  "id":"01H…","ts":1730000000000,
  "text":"Site was shut for the long weekend — confirming with facilities before we roll a truck."}}'
# → {"seq":1}

# 3. The drawer: the record AND the whole thread, newest-first.
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.get","args":{"id":"01H…"}}'
# → {…,"assigned_to":"user:priya",
#     "comments":[{"cseq":1,"text":"Site was shut…","author":"user:ada","ts":…}]}

# 4. My work — resolves to your sub AND every team you're on.
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.list","args":{"assigned_to":"me"}}'

# 5. Bulk: the roster's checkbox gesture. Max 100, PER-ITEM results.
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.assign","args":{
  "ids":["01H…","01J…","bad-id"],"assignee":"user:priya"}}'
# → {"results":[{"id":"01H…","ok":true},{"id":"01J…","ok":true},
#                {"id":"bad-id","ok":false,"error":"bad input: no such insight: bad-id"}]}
# SURFACE THE FAILURES. A green toast over 12 failed rows is the no-silent-caps rule broken at
# the last mile. Over 100 ids is an explicit error — nothing is assigned, nothing is truncated.

# Un-assign: back to the queue.
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.assign","args":{"id":"01H…","assignee":null}}'
```

**The rule that makes it trustworthy:** a re-raise **never** touches either field — including the
re-open arm, where `status_by`/`status_ts` DO clear. A flapping sensor re-firing every 15 minutes
cannot un-assign the technician who took the job, and when a resolved finding fires again months
later, the note explaining last time's false alarm is the first thing the next responder reads.

## 4. Occurrences — the per-insight transaction ring

Every raise appends **one occurrence row** into a capped ring under the insight (last N firings with
their per-firing delta — score, reading, txn ref). The parent's `count`/`first_ts`/`last_ts` are the
**lifetime** truth; the ring is the recent evidence. `count` MAY exceed the stored rows.

```bash
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.occurrences","args":{
  "insight_id":"01H…","limit":50}}'
# → {"items":[{"oseq":150,"ts":…,"severity":"critical","data":{"score":0.71,"txn":"t-88187"}},…]}
```

- **2 KB hard cap on `occurrence.data`** (serialized). Oversize rejects the **whole raise** as
  `BadInput` — never silent truncation, never a partial write (validated up front, no orphan row).
- **Ring cap** default **100** per insight, workspace-admin adjustable in `[0, 1000]` via the policy
  record (`ring_cap`; 0 = occurrences disabled but `count` still increments). Rows evict oldest.
- The occurrence's per-firing `severity` is recorded independently; the parent reflects the newest.
- **`oseq`** (not `seq`) is the wire field — the per-insight monotone number (= the parent's
  post-bump lifetime count). Keyset page strictly **before** `cursor.oseq` (newest-first).

## 5. Subscriptions — push insights into a channel

A member **subscribes a channel** to the insights they care about — all of them, one rule
(`origin_ref`), one identity (`dedup_key`), a **tag facet** (`{siteRef: "building-1"}`), or a
severity floor — without touching the producing flow. Filter axes AND-compose; any subset; all
absent = "all insights in this ws".

```bash
# Subscribe the building-1 ops channel to warning+ insights tagged siteRef=building-1.
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.sub.create","args":{
  "sink":{"kind":"channel","channel":"building-1-ops"},
  "filter":{"tags":{"siteRef":"building-1"},"severity_min":"warning"},
  "now":1719800000000}}'
# → {"id":"01J…"}

# Subscribe a fraud channel to ONE rule's output. identity-only is the same with dedup_key.
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.sub.create","args":{
  "sink":{"kind":"channel","channel":"fraud-alerts"},
  "filter":{"origin_ref":"rule:scorer"},
  "now":1719800000000}}'

# Mute keeps the sub (notify state keeps accumulating) but stops deliveries.
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.sub.mute","args":{"id":"01J…","muted":true}}'
```

- Delivery happens **under the subscriber's stored principal, re-checked at fire time** (the
  reminders pattern). On a deny (member removed, channel grant revoked) the sub flips to a dormant
  state and one final system item is posted to the **owner's inbox** — never a silent stop.
- **`throttle_override`** pins a ladder level: `"immediate" | "hourly" | "daily" | "weekly" |
  "monthly"`. A pager channel pins `immediate`; a summary channel pins `daily`. Pinned subs skip
  escalate/decay but keep breakthroughs + ack-suppression.
- **Hard cap 1,000 subs per workspace** (deny on exceed). `sub.list` is own-only by default;
  `all:true` is the admin lens.

## 6. The digest ladder (anti-spam) + the kill switch

The most-hated failure mode of every alerting system is **spamming people**. Insights delivers
*adaptively by default*: a noisy `(sub, dedup_key)` automatically decays
`L0 immediate → L1 hourly → L2 daily → L3 weekly → L4 monthly`, climbs back when quiet, and always
**breaks through** for genuinely new information.

- **Breakthroughs beat the ladder** (delivered immediately at any level): first-ever occurrence of
  a key on a sub · severity escalation (warning→critical) · re-open after resolve. New information
  is never digested away.
- **Ack means "I know":** while an insight is `acked`, per-key deliveries are suppressed on every
  sub (accounting continues; escalation/re-open still break through).
- **Escalate:** ≥3 deliveries-worth of noise within a window → `level + 1` (a 5-min-firing fault
  reaches daily within its first hour). **Decay:** one fully-quiet window → `level - 1`.
- **Digests are one message per `(sub, window)`** — "⚠ 42 occurrences across 3 insights this day —
  worst: critical `fraud:4421` (31×)" — not N per key. Idempotent per `(sub, window_start)`.

```bash
# Tune the workspace policy (admin). Absent record ⇒ compiled defaults (15 min L0 cooldown,
# ×3 escalation, 100-row occurrence ring, 1000-sub cap).
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.policy.set","args":{
  "l0_cooldown_ms":900000,"escalation_threshold":3,"ring_cap":100,"sub_cap":1000}}'
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.policy.get","args":{}}'

# Per-member kill switch (the whole insight-notification system, for one member). Default true.
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"prefs.set","args":{
  "prefs":{"insight_notifications":false}}}'
```

A digest reactor scans on the injected clock and posts due digests under each sub's stored
principal — exactly one node drives a workspace's digests (owner-election precedent), and a re-run
never double-posts (idempotent item id). The kill switch skips delivery but keeps accounting, so
re-enabling picks up a sane next digest (no replay flood).

## 7. Live feed — `insight.watch` (SSE)

```bash
curl -N http://127.0.0.1:8080/insights/events?token=$TOKEN
# event: message
# data: {"kind":"raise","id":"01H…","dedup_key":"fraud:card-4421","severity":"critical",…}
```

`GET /insights/events?token=<jwt>` — SSE over the bus subject `ws/{ws}/insight/events`. Query-param
auth (EventSource can't send a bearer header); `401` on a bad token; `403` (opaque) without
`mcp:insight.watch:call` or across workspaces (the subject is ws-scoped — no cross-ws leak). The
durable list is `insight.list`'s job; this is the "watch it grow" half — fire-and-forget, a missed
event is not a data loss (re-fetch via `list`).

## 8. The AI analyst — no new agent surface

The shipped **agent dock** rides the Insights page with page context injected; **`builtin.insights-analyst`**
(`extends builtin.data-analyst`) carries the investigation verbs (`insight.get/list/occurrences/ack/
resolve`, `rules.get`) — deliberately NO `insight.raise` (this persona investigates, doesn't mint).
A user opens the dock on the Insights page and asks "why is AHU-2 hunting?" — the persona answers
via `insight.get` → `series.read`/`federation.query` → `rules.get`, under `persona ∩ agent ∩ caller`.
The persona is grounded by this `core.insights` skill.

**Read `analysis` by name; do not paraphrase `body`.** When `insight.get` returns an `analysis`, the
producer has already stated its trigger logic, hypothesis, metric, benchmark, deviation, and impact —
answer from those named fields rather than sniffing the shape of free-form `body` JSON. Two rules when
relaying it: `suspected_cause` is a **hypothesis**, so hedge it in the answer ("the rule suspects…")
rather than reporting it as a diagnosis a site visit can skip; and `estimated_impact` is an
unfalsifiable producer claim with no provenance on the record — attribute it ("the rule estimates
$180/day"), never assert it as measured. When `analysis` is absent, `body` is still the fallback.

**The comment thread is HUMAN testimony — the highest-value context on a re-opened finding, and the
one thing on the record you must not treat as fact.** `insight.get` returns it; read it, because it
is where "we checked this last quarter and it was a shut site" lives, and nothing else on the record
carries that. But attribute every claim to its author and its time ("on the 3rd, Ada noted…") rather
than restating it as the current state — a note is what one person believed then, it is never
corrected in place (the thread is append-only, so a later comment may contradict an earlier one, and
**both remain**), and the finding may have re-opened since. Where a comment and the producer's
`analysis` disagree, say so rather than silently preferring either.

The analyst persona reads triage state but **does not write it**: `insight.assign` and
`insight.comment` are not in `builtin.insights-analyst`'s verb set, for the same reason
`insight.raise` isn't — assigning work to a person and speaking in their operational log are human
acts. Suggest an owner in prose if asked; don't call the verb.

## Gotchas

- **The record's `tags` are an ECHO — read-only, and the graph is the write path.** Every insight
  carries its resolved facets as a flat `{k: v}` map on BOTH `get` and `list` (that is what makes a
  dimension column cheap). It is host-computed like `producer`: a `tags` projection in a raise body
  is ignored, and there is no `insight.tag` verb. To change a finding's dimensions, re-raise with
  the tags declared, or apply them through the `tags.*` verbs — the echo is recomputed from the
  graph on the next raise, so it is the **union across all raises**, and an out-of-band change
  self-heals. It carries no provenance (read `tags.of` for who applied what and when), and
  **filtering never trusts it**: `insight.list {tags}` resolves through the graph. An echo over
  2 KB is skipped whole with a warning rather than truncated. A record raised before the echo
  shipped has none until it next fires — render that as "no dimensions", not as empty truth.
- **`analysis` is a CLOSED struct — a seventh field is dropped without an error.** The six names are
  the contract (§"The six `analysis` fields"); `body` is the overflow. This is the platform's
  most-repeated failure mode, kept deliberately here because a fixed vocabulary is the point — so if
  a field you set never appears in `get`, it is not in the struct, and no error will ever tell you.
- **Three dedup behaviours coexist on one record.** A re-raise refreshes the *producer-owned*
  projections (`severity`, `evidence`, `analysis`, the `tags` echo) **on supply** — omission means
  "leave it alone", never "blank it". It freezes `title`/`body` (first-raise-wins — a known
  inconsistency with a filed follow-up, `scope/insights/insight-prose-refresh-scope.md`). And it
  leaves *human* facts (`assigned_to`, `comments`) untouched **entirely, forever** — there is no
  `assigned_to` on the raise input at all. When reading a long-lived finding, know which class each
  field is in: the reasoning describes the latest firing, the narrative may describe the first.
- **Comments do NOT evict — unlike the occurrence ring they sit beside.** The thread you read is
  complete, not a window, so there is no cursor and no "load older". Both bounds **refuse** instead:
  a comment over 4 KB rejects the call, and appending past 200 comments errors with the existing
  thread untouched (assert your oldest note is still there — it is). This is deliberate: evicting a
  machine-generated firing is housekeeping; evicting a note a person wrote is a trust failure.
  Comments are purged only **with** their insight.
- **`assign`/`comment` do not notify anyone in v1.** The subscription ladder is subject-matched, not
  assignee-matched, so assigning is a *roster fact* only — the assignee is not paged. A UI must not
  imply otherwise. (An `assignee` match arm is the named first follow-up.)
- **A subject outlives its membership.** Assign validates at write time; a member removed later
  leaves insights owned by a subject that can no longer read them. The record deliberately keeps the
  stale value — render an unresolvable assignee as **"unknown (removed)"**, never blank, or the
  orphaned queue is invisible rather than merely stale.
- **A multi-source tag key is currently non-deterministic in the echo.** Edge identity is
  `(entity, tag, source)`, so a `Producer` and a `Human` value for one key coexist and the flat echo
  keeps whichever the graph returned last. The decided rule is `Human` > `Producer`
  (`scope/insights/insight-tag-precedence-scope.md`) but it is **not built** — so do not offer
  re-classification of a producer-set key as if the correction will stick.
- **Identity lives in `dedup_key`/`body`, NEVER the title or tags.** Tags are low-cardinality
  dimensions (site/equip/kind/rule-name) — per-transaction/card identities as tag values blow the
  tag-node cap. `dedup_key: "fraud:card-4421"`, not `tags: {card: "4421"}`.
- **An insight with no matching subscription and no producer-authored sink reaches nobody.** The
  page surfaces "0 subscribers" so a resolved-and-never-delivered insight doesn't become a trust bug.
- **No `update`/`delete` in v1.** Correction = resolve + raise; purge is a future admin batch job
  (the job-retention precedent).
- **`oseq` (not `seq`)** on the occurrence wire — the per-insight monotone number. A `cursor` for
  `occurrences` is `{seq: <oseq>}`; keyset pages strictly before it (newest-first).
- **`insight.sub.create` is double-gated:** the verb cap AND `bus:chan/{channel}:pub` at create
  time, re-checked at every fire. Losing the channel grant flips the sub dormant (owner notified).
- **The origin deep-link** from the detail drawer to the rule/flow/run route is a known follow-up
  (the workspace id isn't threaded into the drawer yet) — the origin is shown, just not yet clickable.
- **The rhai producer handle SHIPS; the flow sink node is the remaining follow-up.** A rule raises a
  durable insight in-body via the `insight` cage handle (`insight.raise`/`ack`/`close`, catalog in
  `lb_rules::CATALOG`, route-aware — no-op on a `route:false` panel run) — see `skills/rules/SKILL.md`
  §7 and the `../../testing/insights/README.md` producer-door check. Agents/extensions/CLI/manual
  reach `insight.raise` via the MCP verb. The built-in `insight` flow **sink node** is still scaffolded
  for the producer-doors follow-up.

## Related

- Scope + shipped doc: `scope/insights/insights-scope.md` (umbrella) +
  `insight-occurrences-scope.md` + `insight-subscriptions-scope.md` + `insight-notify-scope.md`,
  `sessions/insights/insights-session.md`, `public/insights/insights.md`.
- The tag graph that powers facets + subscription filters: `skills/tags/SKILL.md`.
- The channels a subscription posts into: `skills/channels-inbox-outbox/SKILL.md`.
- The agent dock + the analyst persona: `skills/agent/SKILL.md`,
  `scope/agent-personas/persona-catalog-scope.md`.
- Rules (the future `insight.raise(#{…})` rhai handle): `skills/rules/SKILL.md`.
- `README.md` §3 (rules 2/5/6/7 — one datastore, capability-first, workspace wall, MCP contract).
