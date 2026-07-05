# Insights scope — adaptive notify (the anti-spam digest ladder)

Status: scope (the ask). Sub-scope of [`insights-scope.md`](insights-scope.md); promotes to
`public/insights/` with it.

The most-hated failure mode of every alerting system is **spamming people**: a fault that
fires every 5 minutes posts every 5 minutes until humans mute the channel and miss the real
one. This scope makes delivery *adaptive by default*: a noisy insight automatically decays
from immediate posts to hourly → daily → weekly → monthly **digest summaries**, climbs back to
immediate when it goes quiet, and always **breaks through** for genuinely new information
(first occurrence, severity escalation, re-open after resolve). Defaults ship tuned;
everything is adjustable per workspace and per subscription; every member holds a global off
switch.

## Goals

- **The ladder:** per `(subscription, dedup_key)` delivery levels
  `L0 immediate → L1 hourly → L2 daily → L3 weekly → L4 monthly`, escalating on sustained
  noise, decaying one level per fully-quiet window.
- **Breakthroughs beat the ladder** — delivered immediately at any level: first-ever
  occurrence of a key on that sub · severity escalation (warning→critical) · re-open after
  `resolved`. New information is never digested away.
- **Ack means "I know":** while an insight is `acked`, per-key deliveries are suppressed on
  every sub (accounting continues; escalation to `critical` and re-open still break through).
- **Digests are one message**, not N: "⚠ 42 occurrences across 3 insights this day — worst:
  critical `fraud:4421` (31×) — [view]" as a single channel item with a deep link to the
  filtered Insights page.
- **Adjustable with defaults:** one workspace policy record (admin-owned); per-sub
  `throttle_override` pinning a fixed level (a pager channel wants `L0` always; a summary
  channel wants `L2` always); per-sub `muted`; per-member **global kill switch** so a user can
  disable the whole insight-notification system for themselves.

## Non-goals

- **On-call / escalation-to-humans chains** (rotate, page, ack-or-reroute). Different product;
  the outbox + a future extension.
- **Cross-key digest shaping** beyond count/severity/top-keys (clustering, AI summaries) —
  a later tenant; the digest message is deliberately mechanical in v1. (An `ai.*` digest
  narrator is a nice follow-up once digests exist.)
- **Email/webhook digest delivery** — arrives with the outbox sink kind (subscriptions scope
  non-goal, same gate: the email `Target`).

## Intent / approach

A pure, clock-injected **ladder state machine** (`lb-insights`, unit-testable with zero I/O)
+ one durable **digest reactor** at the reminders altitude + one workspace policy record +
one prefs axis. Intents from the subscription matcher flow in; `channel.post` calls flow out
under the sub's stored principal. All timing runs on the **injected logical clock** (the
reminders/cron discipline — deterministic tests, no wall-clock in core).

Rejected alternatives:
- **Fixed rate-limit per sub** ("max 1 post / 15 min"): stops the flood but loses the
  information — drops fire silently instead of accumulating into a summary, and it punishes
  quiet keys for a noisy neighbour (per-sub, not per-key).
- **Producer-side throttling** (the rule sleeps): wrong layer — the record must keep counting
  (the data is the point); only *human delivery* should decay.
- **Reusing reminders** to drive digests: a reminder is a user-authored schedule; digest
  windows are derived state that must move when the ladder moves. Same reactor *pattern*, own
  reactor.

## The state machine

```
insight_notify:{ws}:{sub}:{dedup_key}
{
  level,            // 0..4
  window_start,     // logical ts — start of the current accumulation window
  window_hits,      // raises seen this window
  pending: {        // what the next digest will say (zeroed after send)
    count, first_ts, last_ts, max_severity
  },
  last_sent_ts,
  last_severity     // to detect escalation breakthroughs
}
```

**Defaults** (all on the policy record):

| Level | Window | Behavior |
| --- | --- | --- |
| L0 immediate | cooldown **15 min** | post per raise, but at most one per cooldown per key (extra raises within the cooldown accumulate into the next post) |
| L1 hourly | 1 h | one digest per window with pending counts |
| L2 daily | 24 h | same |
| L3 weekly | 7 d | same |
| L4 monthly | 30 d | same |

- **Escalate:** ≥ **3** deliveries-worth of noise within the current window (i.e. the key kept
  firing past the cooldown/window repeatedly) → `level + 1`. A 5-minute-firing fault reaches
  L2 (daily) within its first hour of life and stops hurting.
- **Decay:** one **fully quiet** window at the current level (zero raises) → `level - 1`.
  Quiet for a day at L2 → back to hourly; quiet again → immediate. A returning fault after a
  quiet period is heard loudly again.
- **Breakthrough** (checked before the ladder): intent kind `reopen`, severity >
  `last_severity`, or no state row yet (first occurrence on this sub) → deliver **now**, keep
  the level unchanged (a breakthrough doesn't reset the ladder — the noise history stands).
- **Ack suppression:** intents for an `acked` insight update `pending`/`window_hits` but never
  deliver; breakthrough rules still apply (escalation/re-open un-suppress by definition —
  re-open flips status to `open`).
- **Muted sub / dormant sub:** accounting continues, delivery skipped (subscriptions scope).
- **Member kill switch off:** deliveries for that member's subs skipped entirely (accounting
  continues so re-enabling picks up sane digests).

## The digest reactor

`react_to_insight_digests` (host, one file, reminders altitude): a durable scan on the
injected clock over `insight_notify` rows whose `window_start + window(level)` has elapsed and
`pending.count > 0` → compose **one digest per (sub, window)** aggregating all that sub's due
keys (not one message per key — the whole point), `channel.post` under the sub's stored
principal (fire-time re-check per the subscriptions scope), zero the pendings, advance
windows, apply decay for quiet keys. Idempotent per `(sub, window_start)` — the digest item id
is derived from it, so a reactor re-run upserts the same Item (the inbox idempotency
contract). Digest message v1 is text + deep link; a `render:` rich table (channels
rich-responses) is the named follow-up.

## Settings surface

- **Workspace policy** — `insight_policy:{ws}` (one record, admin verbs
  `insight.policy.get|set`, caps `mcp:insight.policy.<verb>:call`): ladder windows, cooldown,
  escalation threshold, occurrence ring cap (occurrences scope), sub cap. Absent record =
  compiled defaults (seed pattern: defaults live in code, the record stores overrides only).
- **Per-sub** — `throttle_override` (pin a level: `"immediate" | "hourly" | "daily" | "weekly"
  | "monthly"` — pinned subs skip escalate/decay but keep breakthroughs and ack-suppression)
  and `muted` (subscriptions scope).
- **Per-member kill switch** — a new nullable axis on `lb_prefs::Prefs`
  (`insight_notifications: Option<bool>`, default true) — the shipped whole-fold prefs
  pattern: serde-default flows through, zero host/gateway plumbing beyond the read at
  delivery time.

## How it fits the core

- **Tenancy:** state rows and the reactor scan are ws-scoped; digests only post into same-ws
  channels under the sub's principal.
- **Capabilities:** policy verbs admin-gated; delivery authority is entirely the sub's stored
  principal (this scope adds no authority of its own); deny at fire → the subscriptions
  scope's dormancy path.
- **Placement:** either; the reactor follows the flows/reminders owner-election precedent so
  exactly one node drives a workspace's digests.
- **State vs motion:** ladder state + policy are records; deliveries are `channel.post`
  (durable Item + bus motion). Nothing must-deliver beyond that in v1 — a missed digest window
  is re-driven by the durable scan.
- **Determinism:** all windows on the injected logical clock; the state machine is a pure
  function `(state, intent|tick, policy, now) -> (state', deliveries)` in its own file —
  the unit-test surface.
- **API shape:** policy get/set; no list/watch (the state table is internal — surfaced only
  through digests and the sub's own view); batch N/A.
- **Skill doc:** folded into `skills/insights/SKILL.md` — must show the ladder in action
  (raise ×10 → assert one immediate + one hourly digest) and the kill switch.

## Example flow (the 5-minute nag, tamed)

1. AHU-2 short-cycle rule fires every 5 min. First raise → breakthrough (new key) →
   immediate post in `building-1-ops`.
2. Next 15 min of raises accumulate under the L0 cooldown → one more post with `count: 3` →
   noise threshold hit → L1.
3. The hour's firings become one hourly digest; still firing → L2. From now on: **one daily
   summary** ("short-cycle 288× today, worst warning") instead of 288 messages.
4. An engineer acks it → even the daily digest stops. The compressor degrades, a raise comes
   in at `critical` → **breakthrough**, immediate post despite ack + L2.
5. They fix it, resolve. Two quiet days decay the state L2→L1→L0. A month later it re-opens →
   breakthrough, immediate — heard like new.

## Testing plan

All ladder tests drive the pure state machine + the reactor on an injected clock (rule 9:
real store/channels for the integration layer, seeded raises, no fakes).

- **Capability deny (mandatory):** `insight.policy.set` denied to a member; delivery under a
  revoked sub principal → dormancy, no post.
- **Workspace isolation (mandatory):** ws-A's noisy key never touches ws-B state; policy is
  per-ws; digest posts stay in-ws.
- **Ladder:** escalation at threshold; decay after one quiet window; cooldown accumulation
  (10 raises in 15 min = 1 post + pending 9); pinned override skips escalate/decay.
- **Breakthroughs:** first-key, severity-escalation, re-open — each delivers at L2+; a
  same-severity raise does not.
- **Ack suppression:** acked → digest silent, pending still counts; critical escalation
  breaks through ack.
- **Digest:** one message per (sub, window) covering multiple keys; idempotent on reactor
  re-run (same item id, no duplicate post); deep link filter correct.
- **Kill switch:** prefs axis false → nothing posts; flip back → next window digests include
  the gap (no replay flood — one summary, not N).
- **Determinism:** the same intent sequence + clock always yields the same deliveries.

## Risks & hard problems

- **State-row growth:** one row per (sub, active key) — bounded in practice by dedup, but the
  retention follow-up must sweep rows for resolved-and-quiet keys (e.g. drop state after two
  fully-quiet decay cycles at L0).
- **Tuning fights:** the defaults (15 min / ×3 / one-quiet-window) are opinions; they must be
  policy-record knobs from day one so ops can tune without a release — but the *shape* (ladder
  + breakthroughs) is fixed, or the tests mean nothing.
- **"Where did my alert go?"** — decayed delivery can feel like a lost alert. The digest must
  always say the current level ("daily summary — this key escalated from immediate on …") and
  the insight detail page must show its notify state per sub. Honesty is the mitigation.
- **Reactor duplication across nodes:** without the owner-election discipline two nodes
  double-post digests; the idempotent item id is the backstop, election is the fix.

## Open questions

1. Should the L0 cooldown differ by severity (critical 5 min, info 60 min)? (Recommend: one
   cooldown v1; severity already gets breakthroughs.)
2. Quiet hours / timezone-aware digest send times (post the daily digest at the member's 8am
   via `lb-prefs` tz)? Nice, real, deferred — windows are logical-clock-relative in v1.
3. Should `escalation threshold` count raises or deliveries? (Scoped as deliveries-worth of
   noise; the implementing session should validate feel against the 5-min example.)
4. An `ai.*`-narrated digest ("what changed today, in one paragraph") — follow-up once
   mechanical digests ship.

## Related

- [`insights-scope.md`](insights-scope.md) (umbrella),
  [`insight-subscriptions-scope.md`](insight-subscriptions-scope.md) (intent producer),
  [`insight-occurrences-scope.md`](insight-occurrences-scope.md) (ring cap on the policy
  record)
- `scope/reminders/reminders-scope.md` (durable clock-scan reactor + injected clock),
  `scope/flows/triggers-lifecycle-scope.md` (owner election)
- `scope/prefs/` (the member kill-switch axis), `scope/channels/channels-rich-responses-scope.md`
  (the rich digest follow-up)
- `scope/jobs/job-retention-scope.md` (state-sweep precedent)
