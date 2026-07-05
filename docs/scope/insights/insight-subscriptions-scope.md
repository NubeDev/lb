# Insights scope — subscriptions (listen to a rule, an identity, a tag facet, or everything)

Status: scope (the ask). Sub-scope of [`insights-scope.md`](insights-scope.md); promotes to
`public/insights/` with it.

The umbrella scope made delivery *explicit authoring* — the flow that raises an insight also
posts to a channel if it wants one. That's right for producers, but consumers need the inverse:
**a member subscribes a channel to the insights they care about** — all of them, one rule's,
one identity's (`dedup_key`), a severity floor, or a **tag facet** ("everything on
`siteRef:building-1`", "anything tagged `kind:energy`") — without touching the producing flow.
This scope adds that subscription record and the raise-time matcher. What happens *after* a
match (send now vs digest) is the notify scope's job.

## Goals

- One **subscription record** owned by a member: a filter over insights + a channel sink.
- Filter axes, AND-composed, any subset: `all` (empty filter) · `origin_ref` (a rule/flow) ·
  `dedup_key` (an identity) · `tags` facets (the tag graph does the organising) ·
  `severity_min`.
- **Raise-time matching** in the host: every raise is evaluated against the workspace's
  subscriptions and produces *notification intents* — never direct posts (the notify ladder
  decides delivery).
- Delivery happens **under the subscriber's stored principal, re-checked at fire time** (the
  shipped reminders pattern) — a subscription can never post where its owner can't.
- Per-subscription `muted` switch; the member-global kill switch lives in the notify scope.

## Non-goals

- **No outbox/email sink in v1.** Channel-only; an `outbox` sink kind is additive once the
  email `Target` exists (umbrella scope's named gap).
- **No per-subscription UI feed.** The Insights page is the browse surface; subscriptions are
  about *push into channels*.
- **No shared/team-owned subscriptions in v1.** A member subscribes a channel; a team wanting
  coverage subscribes via any member with the channel grant. Team-owned subs follow the share
  model later if demanded.

## Intent / approach

A small record + CRUD verbs + one pure matcher function called on the raise path. Matching is
synchronous and cheap: the workspace's subscription set is small (hard cap **1,000 per
workspace**, deny on exceed — the tags 10k-cap pattern), loaded through the store with the
existing read path, and each filter check is field equality / severity ordering / tag-subset.
The raise verb hands matches to the notify engine as intents; it never blocks on channel I/O.

Rejected alternatives:
- **Subscriptions as bus subscriptions** (`insight.watch` + client-side filter): motion-only —
  misses offline members, can't feed digests, and pushes the filter to every client. The bus
  event stays for live UI; subscriptions are durable state.
- **Subscriptions on the producing flow** (a "notify" list on the flow record): couples
  consumers to producers — a building manager shouldn't need edit rights on the fraud flow to
  follow `siteRef:building-1`.
- **A rules-engine rule per subscription** ("when insight matches X, channel.post"): Turing
  overkill for five filter fields, and it would bypass the anti-spam ladder.

## The record

```
insight_sub:{ws}:{id}
{
  id,                       // ulid
  owner,                    // member subject
  principal,                // stored caps snapshot, RE-CHECKED at fire (reminders pattern)
  sink: { kind: "channel",  // v1: channel only
          channel },        // target channel id
  filter: {                 // AND of every provided field; all absent = "all insights"
    origin_ref?,            // e.g. the rule id — "subscribe to this rule"
    dedup_key?,             // e.g. "fraud:4421" — "subscribe to this identity"
    tags?,                  // { k: v, ... } — insight must carry ALL (tag-facet subset)
    severity_min?           // "info" | "warning" | "critical"
  },
  muted,                    // bool — keep the sub, stop deliveries (state still accumulates)
  throttle_override?,       // notify-scope: pin a ladder level (e.g. always "daily")
  created_ts
}
```

Tag matching is a subset check against the insight's tag edges (the same `tag:[key,value]`
nodes `tags.find` facets on) — the tag graph is the organising layer, exactly as the umbrella
scope uses it for the page facets. Low-cardinality discipline carries over unchanged.

## Verb surface

- `insight.sub.create { sink, filter, throttle_override? }` → `{ id }` — requires the caller
  hold `bus:chan/{channel}:pub` **at create time** (no-widening up front) in addition to the
  verb cap; the stored principal is the caller's.
- `insight.sub.list {}` — the caller's own subs (admin lens: all ws subs, own cap).
- `insight.sub.get { id }`, `insight.sub.delete { id }` — owner (or admin) only.
- `insight.sub.mute { id, muted }` — owner only.

Caps: per-verb `mcp:insight.sub.<verb>:call`, member-level; deny opaque.

## The raise-time matcher

In `crates/host/src/insight/` (one responsibility per file — `match_subs.rs`): pure function
`(insight_view, subs) -> Vec<Intent>`. Called after the record write + occurrence append +
bus event, inside the same raise handling; each `Intent = { sub_id, insight_id, dedup_key,
severity, kind: raise|reopen|escalate }` is handed to the notify engine (which owns all
send/hold decisions and the actual `channel.post` under the stored principal). A muted sub
still produces intents (the notify state keeps accumulating so an unmute doesn't lose the
digest); the notify engine drops the delivery, not the accounting.

**Fire-time re-check + honest dormancy:** at delivery the stored principal is re-authorized
for `bus:chan/{channel}:pub`. On deny (member removed, channel grant revoked) the sub flips to
`muted` with a `dormant_reason`, and one final system item is posted to the *owner's* inbox
(not the channel) saying the subscription went dormant — never a silent stop.

## How it fits the core

- **Tenancy:** subs keyed `{ws}`; the matcher only ever sees same-ws insights and subs.
- **Capabilities:** per-verb caps; create-time AND fire-time channel `pub` check; the matcher
  itself adds no authority (intents deliver under the stored principal).
- **Placement:** either; matching is in-process on the raise path, no reactor here (digest
  timing is notify's reactor).
- **Data:** one `insight_sub` table + the notify state (next scope). State only.
- **Bus:** none new — deliveries are `channel.post` (already durable Item + motion).
- **API shape:** CRUD + list; no watch (subs change rarely; the page re-fetches); batch N/A.
- **Skill doc:** folded into `skills/insights/SKILL.md` — must include "subscribe a channel to
  a tag facet" and "subscribe to one identity" runs.

## Example flow

1. Building manager: `insight.sub.create { sink: { kind:"channel", channel:"building-1-ops" },
   filter: { tags: { siteRef: "building-1" }, severity_min: "warning" } }`.
2. Fraud lead: `filter: { origin_ref: "rule:fraud-score" }` into `#fraud-alerts`; an analyst
   chasing one card adds `filter: { dedup_key: "fraud:4421" }` into their own channel.
3. A raise tagged `{ siteRef:"building-1", kind:"short-cycle" }` at `warning` matches sub 1 →
   one intent → the notify ladder decides now-vs-digest → `channel.post` into
   `building-1-ops` under the manager's principal, with a deep link to the insight.
4. The manager leaves the workspace → next delivery re-check denies → sub goes dormant, owner
   inbox note posted.

## Testing plan

- **Capability deny (mandatory):** each `insight.sub.*` verb; create denied when the caller
  lacks the channel `pub`; fire-time deny → dormant + owner inbox note, no channel post.
- **Workspace isolation (mandatory):** ws-B subs never match ws-A raises; ws-B cannot
  list/delete ws-A subs.
- **Matcher semantics:** each axis alone; AND composition; empty filter = all; tag-subset
  (insight with extra tags still matches); severity ordering; non-match produces nothing.
- **Mute:** muted sub delivers nothing but notify state still accumulates (assert digest after
  unmute includes the muted-period counts).
- **Cap:** 1,001st sub denied.
- **Ownership:** non-owner delete/mute denied; admin lens works.

## Risks & hard problems

- **Raise-path latency.** O(subs) matching on every raise — fine at the 1k cap with an
  in-memory ws cache, but the cache must bust on sub CRUD (single-node bust + lazy expiry, the
  webhook-revoke precedent; multi-node broadcast is the same named follow-up).
- **Stored-principal staleness.** The reminders pattern's known trade: between grant revoke
  and next fire the sub still *believes* it can post; the fire-time re-check is the wall, and
  the dormancy note keeps it honest.
- **Filter foot-guns.** `all` + a busy workspace = exactly the spam problem — which is why
  intents MUST route through the notify ladder; there is no bypass path.

## Open questions

1. Should `filter.origin_ref` accept a trailing-`*` glob (all rules of a family), matching the
   persona `granted_tools` grammar? (Recommend: exact-only v1.)
2. Team-owned subscriptions via the share-edge model — wait for demand?
3. Should a sub be able to target the **owner's inbox** (`sink.kind:"inbox"`) as a
   channel-free personal feed? Cheap to add (same Item plane) — recommend yes if it falls out
   free, else follow-up.

## Related

- [`insights-scope.md`](insights-scope.md) (umbrella),
  [`insight-notify-scope.md`](insight-notify-scope.md) (what happens to an intent)
- `scope/reminders/reminders-scope.md` (stored-principal-re-checked-at-fire pattern)
- `scope/tags/tags-scope.md` (facet matching), `scope/channels/channels-scope.md`
  (`channel.post`, `bus:chan/{cid}:pub`)
- `scope/ingest/webhooks-scope.md` (revoke cache-bust precedent)
