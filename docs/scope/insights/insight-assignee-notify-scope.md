# Insights scope — assignee notification (the triage plane's missing arm)

Status: **shipped** — session
[`insight-assignee-notify-session.md`](../../sessions/insights/insight-assignee-notify-session.md),
promoted to `doc-site/content/public/insights/insights.md`. See "Open questions after building".

The triage plane shipped ([`insight-triage-scope.md`](insight-triage-scope.md)) with a gap it named
loudly: **assigning gives someone work and tells them nothing.** The subscription ladder is
*subject-matched* — origin, dedup key, tags, severity — and has no notion of who owns a finding, so
"notify me about anything assigned to me" is unexpressible. This scope closes triage's resolved
decisions **1** (notify on assignment) and **5** (`assigned_to` becomes subscription-filterable),
which are the same missing match arm seen from two directions.

## Goals

- **Tell someone they were given work.** An assignment produces a notification into a subscribed
  channel — the thing a person actually wants from this feature.
- **Let a subscription filter by owner.** `SubFilter.assignee` as a first-class AND axis, so
  "anything assigned to my crew, when it fires" is one subscription.
- **Resolve `"me"` the way the roster does** — the sub's owner *and every team they are on*, so a
  queue-assigned finding reaches the person on the queue.
- Change **nothing** for any existing subscription.

## Non-goals

- **Not a notification for every triage act.** Comments do not notify in this scope (see "Rejected"),
  and neither does un-assignment.
- **Not a new sink.** Channel-only, exactly as subscriptions ship today; an email/push target is the
  outbox's job and rides the existing `SubSinkKind` extension point.
- **Not per-assignee delivery preferences.** Who hears about what is expressed by *subscriptions*,
  which is the plane that already exists for it. No second grammar.
- **Not a re-notification on re-raise of an already-assigned finding.** That is the raise path's
  normal ladder behaviour and is already correct.

## Intent / approach

Two capabilities, deliberately separate because they fire at different times:

**A. `SubFilter.assignee` — a raise-time match axis.** Ordinary AND filter beside `tags`/`severity_min`,
evaluated by the pure `match_subs` on every raise. Answers *"a finding my crew owns just fired again"*.
`InsightView` gains `assigned_to`; the filter accepts a subject (`user:…`/`team:…`) or the literal
`"me"`.

`"me"` cannot be resolved inside `match_subs` — it is pure and team membership is a store read — so
the **host resolves it, keyed by sub owner**, and passes the map in. This mirrors exactly what
`insight_list` already does for the roster's `assigned_to: "me"`, and keeps the matcher a pure
function (its whole value as a unit-test surface). Resolution happens **at fire time, not at create
time**: storing the expanded subject set on the subscription would silently stop matching when
someone joins a team.

**B. Assignment-time notification — and it deliberately BYPASSES the ladder.**

This is the load-bearing decision. The ladder is per-`(sub, dedup_key)` anti-spam for **machine
flapping**: escalate on sustained noise, decay on quiet, one L0 post per cooldown per key. An
assignment is none of those things — it is a **one-shot act by a human**, rate-limited by the human
doing it, and it is about *a person receiving work* rather than *a finding firing*.

Pushing it through the ladder is actively wrong, not merely redundant: ladder state is keyed by the
insight's `dedup_key`, so a **flapping finding's own cooldown would suppress "you have been assigned
this"** — the one message that must not be swallowed. The two signals share a key and mean unrelated
things.

So an assignment delivery is emitted directly by the assign path. Everything that makes a delivery
*safe* still applies, because none of it lives in the ladder: the sub's `muted` flag, the owner's
per-member kill switch, `dormant_reason`, and above all the **fire-time re-check of
`bus:chan/{channel}:pub`** under the sub's stored principal (a revoked grant flips the sub dormant and
notes the owner's inbox). Only the *throttling* is skipped, and only because the throttle is modelling
the wrong thing.

**Bulk assign coalesces to ONE delivery per subscription.** "Assign these 12 to Priya" is one human
gesture and must produce one notification (`"12 insights assigned to team:mechanical"`), not twelve.
The bulk verb already holds every id, so the coalescing is free at the call site — which is precisely
why this does not need the ladder's accumulator. Twelve messages for one click is the failure this
whole notify plane exists to prevent.

**Opt-in only: a subscription receives assignment notifications ONLY if it filters on `assignee`.**
A sub with no `assignee` axis is asking about *findings*, and must not start receiving a new event
class it never requested — that would change the meaning of every subscription in every workspace on
upgrade. This makes the feature strictly additive and is the reason no existing sub's behaviour moves.

**Self-assignment does not notify.** If the assigning principal *is* the assignee, the delivery is
suppressed: "I'll take this" is the most common triage gesture and telling someone about their own
action is exactly the noise that makes people mute a channel. Assigning to a **team you are on** still
notifies — the assignee is the queue, not you, and the rest of the crew needs to know.

## How it fits the core

- **Tenancy / isolation:** unchanged. Subs, insights, memberships, and teams are all
  workspace-namespaced; the assignment notify path reads only within the assigning workspace, and the
  `"me"` expansion resolves the sub owner's teams **in that workspace** (README §7).
- **Capabilities:** **no new capability.** Assignment notification is a consequence of
  `mcp:insight.assign:call` (already held to assign) plus the subscription's own already-gated
  existence. The delivery re-checks `bus:chan/{channel}:pub` at fire time exactly as a raise delivery
  does — so this widens nobody's reach: a caller who cannot assign produces no notification, and a sub
  whose owner lost channel access goes dormant instead of delivering.
  - **The deny surface worth testing:** an assign that a caller is denied must produce **no**
    notification (the deny precedes the write, so there is nothing to notify about), and a sub whose
    channel grant was revoked must flip dormant rather than post.
- **Placement:** either — a plain local write + a channel post, exactly like the raise path's
  immediate deliveries. No reactor, no owner election.
- **MCP surface:** **no new verb.** `insight.sub.create`'s `filter` gains an optional `assignee`
  field; `insight.assign` gains a side effect. `insight.sub.get`/`.list` echo the new axis for free.
- **Data (SurrealDB):** one new optional field on the existing `insight_sub` record. **No new table,
  no notify-state rows** — an assignment delivery is stateless by construction (nothing to accumulate,
  since bulk already coalesced and there is no flapping to suppress).
- **Bus (Zenoh):** the existing channel-post path; the shipped `insight/events` subject is unchanged
  (an assign already emits `EventKind::Assign` there for live-UI motion — that is presentation, this
  is delivery).
- **Sync / authority:** ordinary workspace data, node-local, like every other subscription delivery.
- **Secrets:** none.
- **SDK/WIT impact:** none — an additive optional filter field; an old client deserializes unaffected.
- **Skill doc:** **YES** — extend `skills/insights/SKILL.md` with the "notify me about my queue"
  walkthrough, and correct the triage section's standing "assigning notifies nobody" caveat.

## Rejected alternatives

- **Rejected: run assignment through the ladder with a new `IntentKind::Assign`.** The tidy-looking
  option, and wrong for the reason above — ladder state is keyed by `dedup_key`, so a noisy finding's
  cooldown would eat the assignment message. Keying assignment state by *assignee* instead would fix
  that but invents a second meaning for a table whose every field (`last_severity`, `window_hits`,
  escalate/decay) describes a firing, to buy throttling that bulk coalescing already provides.
- **Rejected: notify every subscription that matches the insight, not just assignee-filtered ones.**
  One line simpler and it silently repurposes every existing subscription in every workspace into an
  assignment feed on upgrade. Opt-in is the only additive choice.
- **Rejected: a dedicated `insight.notify_assignee` verb or a `notify: true` flag on assign.** Both
  put the "who hears about this" decision at the *call site*, where the assigning operator would be
  choosing on the assignee's behalf. Subscriptions are the plane that already answers this question,
  owned by the person who wants the notification.
- **Rejected: notifying on comment in the same slice.** Tempting (it is the other half of triage), but
  the two have different shapes: comments are per-thread and chatty, so they want digesting — the
  ladder's actual job — while assignment is one-shot. Bundling them would force one delivery model
  onto both. Filed as the follow-up below.

## Example flow

1. Priya subscribes her crew's channel to her queue:
   `insight.sub.create { sink: { kind: "channel", channel: "chan/mechanical" },
   filter: { assignee: "me", severity_min: "warning" } }`. `"me"` will resolve, at every fire, to
   `user:priya` **plus** `team:mechanical` (she is on the crew).
2. An operator triaging the roster bulk-assigns 12 open findings to `team:mechanical`.
   Each is checked against the sub's full filter (assignee **and** the severity floor); 9 match.
3. **One** delivery lands in `chan/mechanical`: *"9 insights assigned to team:mechanical"*. Not nine.
4. The operator assigns a 13th finding to *themselves*. **No delivery** — self-assignment is silent.
5. Overnight one of the 9 re-fires. The raise-time matcher now matches the same sub on its `assignee`
   axis, so it flows through the **normal ladder** — L0 immediate, then digesting if it flaps. The two
   paths coexist: assignment told her it became hers; the ladder tells her it is still misbehaving.
6. Priya's channel `pub` grant is revoked. The next delivery of either kind flips the sub dormant and
   drops a note in *her* inbox — never a silent stop.

## Testing plan

Per `scope/testing/testing-scope.md`, against the **real** store (`mem://`), real bus, real caps, and
real seeded memberships/teams — no mocks (rule 9). Mandatory categories:

- **Capability deny (mandatory).** A caller denied `mcp:insight.assign:call` produces **no**
  notification (nothing was assigned). A sub whose owner lost `bus:chan/{channel}:pub` **flips
  dormant** and posts nothing, with the owner's inbox note. Revert-check the fire-time re-check by
  gutting it and watching the dormant test go red.
- **Workspace isolation (mandatory).** A ws-B subscription never receives a notification for a ws-A
  assignment, and `"me"` resolves a sub owner's teams **only within the sub's own workspace** (a
  same-named team in another workspace must not widen the match).

Key cases:

1. **Opt-in only.** A sub with no `assignee` axis receives **nothing** on assignment — asserted
   alongside a sub that *does* filter on assignee receiving exactly one. This is the upgrade-safety
   test; it must be impossible to pass by accident.
2. **Bulk coalescing.** Assigning N insights in one call produces exactly **one** delivery per
   matching sub, naming N — and N is the count that actually matched the sub's *full* filter, not the
   number of ids passed.
3. **`"me"` resolves owner + teams.** A finding assigned to `team:mechanical` notifies a sub owned by a
   crew member filtering `assignee: "me"`; a non-member's identical sub gets nothing. The case a naive
   owner-equality check silently drops.
4. **Self-assign is silent**, and assigning to a team the actor is on is **not**.
5. **The ladder is bypassed, provably.** An insight whose `(sub, dedup_key)` ladder state is deep in
   cooldown/L4 still delivers its assignment notification — the assertion that pins the whole design
   decision. Revert-check by routing assignment through `apply_intents` and confirming red.
6. **Muted / kill-switch / dormant subs receive nothing**, exactly as for raise deliveries.
7. **Un-assign notifies nobody** (`assignee: null`).
8. **Raise-time `assignee` matching** composes with the other axes, matches `user:`/`team:`/`"me"`,
   and an insight with no assignee never matches a sub that filters on one.
9. **Live verification in the product** — assign from a running node and observe the channel post
   arrive (and observe the bulk case posting once), not just the suite.

## Risks & hard problems

- **Bypassing the ladder is the risk, and it is deliberate.** Nothing throttles assignment
  notifications except the coalescing at the call site and the humans doing the assigning. A script
  holding `insight.assign` that reassigns in a loop *can* post per call. Accepted for v1: the same is
  true of any verb a script can call, the blast radius is one channel, and the alternative (ladder
  state keyed on a firing) breaks the message that matters most. **If this bites, the fix is a
  per-(sub, assignee) cooldown at the assign path — not re-using the firing ladder.**
- **`"me"` is resolved per fire, so it costs a team read per assign.** Bounded (one resolution per
  distinct sub owner per call, and only for subs that filter on assignee), but it is a store read on a
  write path. If the sub count grows, cache per call — never per process, or team changes stop
  applying.
- **A team-assigned finding can notify many people at once**, which is the point, and also means one
  bulk assign to a large team is a broadcast. The channel is the blast radius and it is the one the
  subscriber chose.
- **The count in the message can be stale by the time it is read** (someone re-assigns in between).
  The delivery is motion, not state — the roster is the truth. The message should read as an event
  ("9 insights were assigned to…"), never as a current count.
- **Two notifications for one finding are now possible** — the assignment message and, later, the
  raise-time ladder delivery for the same sub. That is correct (they say different things) but a UI
  showing both in one channel should make the distinction legible, or it reads as a duplicate.

## Resolved decisions

1. **Assignment notification bypasses the ladder entirely** (see "Intent"). The ladder models machine
   flapping keyed per finding; assignment is a one-shot human act about a person. Sharing the key
   would let a flapping finding suppress the assignment message.
2. **Bulk assign emits one delivery per subscription, naming the count.** One gesture, one
   notification. Coalescing happens at the call site, which is what makes decision 1 affordable.
3. **Only subscriptions that explicitly filter on `assignee` receive assignment notifications.**
   Strictly additive; no existing subscription changes behaviour on upgrade.
4. **Self-assignment does not notify; assigning to a team you are on does.** The assignee is the
   queue, not you, and the crew needs to know.
5. **Un-assignment does not notify.** The feature is "you have been given work"; losing it is not news
   worth a channel post, and a v1 that notifies on both doubles the volume for the weaker half.
6. **`"me"` resolves at fire time, never at create time.** A stored expansion silently stops matching
   when team membership changes — a subscription that quietly goes deaf is worse than one that costs a
   read.
7. **Comment notification is out of scope and filed.** Comments are chatty and per-thread, so they
   want the ladder's digesting; assignment is one-shot and does not. One delivery model cannot serve
   both, and guessing now would pick the wrong one for whichever ships second.

## Open questions after building

Written by the implementing session.

1. **Nothing throttles assignment at all** (the accepted cost of resolved decision 1). Bulk
   coalescing bounds one call; nothing bounds a caller making many. If it bites, add a
   per-`(sub, assignee)` cooldown **at the assign path** — re-using the firing ladder is the trap
   this design exists to avoid.
2. **Two notifications for one finding are now possible** — the assignment, then later a raise-time
   ladder delivery to the same sub. Correct (they say different things), but a UI showing both in one
   channel should distinguish them or it reads as a duplicate.
3. **The opt-in makes the feature invisible.** A member who assigns work will reasonably assume the
   assignee was told; they were not, unless someone had already created an assignee-filtered
   subscription. The UI should say so at the moment of assigning, or "I assigned it, they never
   heard" becomes the new version of the gap this scope closed.
4. **`"me"` costs a team read per assign, per distinct owner using it.** Bounded and lazy today. If a
   workspace grows many `"me"` subs, cache **per call** — never per process, or team changes stop
   applying (decision 6).
5. **Comment notification is still unbuilt** (decision 7) and is the obvious next ask now that
   assignment is wired. It needs the ladder, not this path.

## Related

- Parent: [`insight-triage-scope.md`](insight-triage-scope.md) — **resolved decisions 1 and 5 land
  here**; its "assigning notifies nobody" known gap is closed by this scope.
- [`insight-subscriptions-scope.md`](insight-subscriptions-scope.md) — the filter grammar this extends
  and the fire-time re-check contract it reuses.
- [`insight-notify-scope.md`](insight-notify-scope.md) — the ladder this deliberately bypasses, and
  why (§"The state machine" is the thing being argued with).
- [`insights-scope.md`](insights-scope.md) — the umbrella (the "0 subscribers" trust flag this feature
  is the assignee-shaped instance of).
- `scope/testing/testing-scope.md` §0 (no mocks), `scope/debugging/debugging-scope.md`
- README §3 (rules 5–7: capability-first, workspace wall, MCP contract)
