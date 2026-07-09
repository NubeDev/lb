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
| `insight.raise` | `dedup_key, severity, title, body?, origin, tags?, occurrence?, ts` | `{id, status, count, created, reopened, dedup_key, severity, kind}` (idempotent on `(ws, dedup_key)`) |
| `insight.get` | `id` | the full record |
| `insight.list` | `status?, severity?, origin_ref?, tags?, range?, cursor?, limit?` | `{items:[Insight], next?}` (newest-first, keyset-paged) |
| `insight.ack` | `id, ts` | `{ok:true}` (`open → acked`) |
| `insight.resolve` | `id, note?, ts` | `{ok:true}` (`* → resolved`, idempotent) |
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
  is the retention follow-up's admin batch job.

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

# 3. list — open critical insights, keyset-paged
curl -s -X POST $BASE "${auth[@]}" -d '{"tool":"insight.list","args":{
  "status":"open","severity":"critical","limit":50}}'

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

## Gotchas

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
