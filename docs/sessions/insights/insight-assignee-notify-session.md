# Session — insights: assignee notification (closing triage's named gap)

Status: **shipped**. Scope:
[`../../scope/insights/insight-assignee-notify-scope.md`](../../scope/insights/insight-assignee-notify-scope.md)
(written this session, then built).
Branch: `master` (no commits made — the human owns git).
Date: 2026-07-30.

## The ask, restated

The triage plane shipped able to **give someone work and tell them nothing**. The subscription ladder
is subject-matched — origin, dedup key, tags, severity — with no notion of who owns a finding, so
"notify me about anything assigned to me" was unexpressible. This closes
[`insight-triage-scope.md`](../../scope/insights/insight-triage-scope.md)'s resolved decisions **1**
(notify on assignment) and **5** (`assigned_to` becomes subscription-filterable) — the same missing
match arm seen from two directions.

## What shipped

| Layer | Change |
|---|---|
| Filter | `SubFilter.assignee: Option<String>` + `ASSIGNEE_ME` — a subject or `"me"` (`subscription.rs`) |
| Matcher | `InsightView.assigned_to`, `OwnerSubjects` resolution seam, and the shared `assignee_matches` both paths use (`match_subs.rs`) |
| Raise path | builds the owner-subject map and passes it to `match_subs` (`host/insight/raise.rs`) |
| Assign path | `notify_assignment` — the new delivery, deliberately outside the ladder (`host/insight/assign_notify.rs`) |
| Assign verb | takes `ts` (optional on the wire, host-backfilled like `raise`); notifies only on a real owner **change** |
| Crate verb | `assign` returns `AssignOutcome { assigned_to, changed }` so an idempotent re-assign can't re-notify |
| Host helper | `owner_subjects_for` — resolves `"me"` per distinct sub owner, only for subs that use it |

**No new verb, no new capability, no new table.** One additive optional field on `insight_sub`.

## Decisions made while building

**1. Assignment bypasses the ladder — the load-bearing call.** `ladder_step` is per-`(sub, dedup_key)`
anti-spam for *machine flapping*. An assignment is a one-shot human act about *a person receiving
work*, not about a finding firing. Routing it through the ladder keys it by the insight's
`dedup_key`, so **a flapping finding's own cooldown would swallow "you have been assigned this"** —
the one message that must not be suppressed. Everything that makes a delivery *safe* still applies
(muted, kill switch, dormant, and the fire-time `bus:chan/{channel}:pub` re-check), because none of
that lives in the ladder. Only the throttling is skipped, and only because the throttle models the
wrong thing.

**2. Bulk coalesces to ONE delivery per subscription, naming the count.** One human gesture, one
notification. The verb holds every id, so coalescing is free at the call site — which is exactly what
makes decision 1 affordable: the volume problem the ladder would have solved is solved better, and
earlier, by not fanning out in the first place.

**3. Opt-in only.** A subscription receives assignment notifications **only** if it filters on
`assignee`. A sub without that axis is asking about *findings* and must not silently become an
assignment feed on upgrade. This is what makes the whole feature strictly additive.

**4. Self-assignment is silent; assigning to a team you are on is not.** "I'll take this" is the most
common triage gesture, and telling someone about their own action is the noise that makes people mute
a channel. Assigning to a queue is different — the assignee is the team, and the crew needs to know.

**5. `"me"` resolves at fire time, per sub owner, never at create time.** A stored expansion silently
stops matching when someone joins or leaves a team. Cost is bounded: one resolution per *distinct
owner* of a sub that actually uses `"me"`, and **zero** reads in a workspace where nobody does.

**6. An idempotent re-assign does not re-notify** (added while fixing a test, see below). `assign` now
reports whether it actually changed the owner; a double-click or retried bulk call writes nothing and
announces nothing. A retry that pages a queue twice is a duplicate, not an event.

**7. Un-assignment notifies nobody.** The feature is "you have been given work"; losing it is not
news worth a channel post, and notifying both halves doubles volume for the weaker one.

## Testing

`rust/crates/host/tests/insight_assignee_notify_test.rs` — **16 cases**, real booted `Node`, real
store, real bus, real caps, real subscriptions, real seeded memberships/teams, and real delivered
inbox Items read back through the real `inbox.list` verb. No mocks (CLAUDE §9).

```
cargo test -p lb-host --test insight_assignee_notify_test → 16 passed
                      --test insight_triage_test          → 17 passed  (slice 3, unbroken)
                      --test insights_test                → 22 passed  (the notify plane it extends)
                      --test insight_analysis_test        → 16 passed
                      --test insight_tag_echo_test        → 11 passed
                      --test insight_evidence_test        → 10 passed
cargo build --workspace → green;  cargo fmt --all --check → clean
```

Mandatory categories: **capability-deny** (a denied assign notifies nobody; a sub whose channel grant
was revoked **flips dormant** rather than posting, with the owner's inbox note) and
**workspace-isolation** (a ws-B sub never hears a ws-A assignment, and `"me"` resolves teams only
within the sub's own workspace — asserted with a same-named team in both).

### Two tests that passed for the wrong reason — the real story of this session

Both revert-checks the scope names **failed to bite on the first attempt**, and each exposed a test
that was green without testing what it claimed. Recording this because the tests looked fine.

**a. The opt-in gate was enforced twice.** I removed the explicit
`filter(|s| s.filter.assignee.is_some())` expecting `only_subs_that_filter_on_assignee_hear_an_assignment`
to go red. It stayed green: `unwrap_or_default()` produced an empty `want` string that matched no
assignee, so the opt-in held by accident. The gate *looked* removable when it was load-bearing.
Fixed by collapsing to a single explicit `let Some(want) = … else { continue }` — one place where the
opt-in is decided. The revert-check then failed correctly, on exactly that one test.

**b. The ladder-bypass test used two subscriptions.** It flapped a key to heat the ladder, then
assigned — but the flapping heated the `findings` sub's state while the assignment went to the `q`
sub. Different `(sub, dedup_key)` keys, so routing assignment through `apply_intents` was a *first-key
breakthrough* and delivered anyway. The test's own comment claimed "one sub that matches BOTH", which
the code did not do. Rebuilt to construct the collision properly: assign first (so the
assignee-filtered sub also matches on the raise path), flap the key (heating *that* sub's state for
*that* key), then change the owner again. Counting also had to become assignment-specific
(`assign_posts`), since that one sub now receives both kinds. The revert-check then failed correctly.

Fixing (b) is what surfaced decision 6 — the un-assign/re-assign construction only works if an
idempotent re-assign is *not* an event, which it wasn't yet.

### Revert-checks (after the fixes above)

| Revert | Result |
|---|---|
| Treat an absent `assignee` filter as match-all (drop the opt-in) | `only_subs_that_filter_on_assignee_hear_an_assignment` red, nothing else. Restored. |
| Route assignment through `apply_intents` (the firing ladder) | `an_assignment_delivers_even_when_that_subs_ladder_is_in_cooldown_for_that_key` red, nothing else. Restored. |

## Live verification (not just the suite)

Real booted node from this tree, over the real `POST /mcp/call` wire (`{tool, args}`):

```
setup: team:mechanical + two subs — "queue" {assignee:"team:mechanical"}, "everything" {}

bulk assign 3 findings to team:mechanical (ONE gesture)
  → {"results":[3 × ok:true]}
  queue      → 1 post: “3 insights were assigned to team:mechanical [view]”   ← coalesced
  everything → 3 posts, 0 of them assignment posts                            ← opt-in holds

single assign  → “insight solo — “solo finding” was assigned to team:mechanical [view]”
re-assign ×3 to the SAME owner → assignment posts still 2                     ← no duplicate paging
ada assigns to user:ada, sub {assignee:"me"} → 0 posts                        ← self-assign silent
un-assign      → assignment posts still 2                                     ← un-assign is silent
```

Every resolved decision confirmed end to end.

**Honest gap:** the `"me"`-resolves-**through-teams** path is covered by the suite (with real team
edges) but the live run exercised `"me"` only for the self-assign case — the dev-login token lacks
`members.manage`, so I could not add a second member to a team over the wire. Same limitation the
triage session hit; the suite covers it with real rows rather than fixtures.

## What this closes

- `insight-triage-scope.md` resolved decisions **1** and **5** — its "assigning notifies nobody"
  known gap is now closed in the scope, the session log, the public doc, and the skill doc.
- The umbrella's "0 subscribers" trust flag now has its assignee-shaped instance answered: a queue
  can subscribe to itself.

## Filed, not fixed

- **Comment notification.** Deliberately out of scope (resolved decision 7): comments are chatty and
  per-thread, so they want the ladder's *digesting* — the opposite of assignment's one-shot shape. One
  delivery model cannot serve both, and guessing now would pick the wrong one for whichever ships
  second. Worth its own scope when someone asks for it.
- **No throttle on assignment.** Accepted for v1 (scope §Risks): a script holding `insight.assign` in
  a loop can post per call. The blast radius is one channel and the rate is human-shaped in practice.
  **If it bites, the fix is a per-`(sub, assignee)` cooldown at the assign path — not re-using the
  firing ladder**, which is the trap this whole design avoided.
- **Two notifications for one finding are now possible** (the assignment, then later a raise-time
  ladder delivery to the same sub). Correct — they say different things — but a UI showing both in one
  channel should make the distinction legible or it reads as a duplicate.

## Still open from earlier slices

Unchanged by this session: the tag-echo **backfill job**, `tags.*` having **no dispatcher entry**, and
[`insight-tag-precedence-scope.md`](../../scope/insights/insight-tag-precedence-scope.md) (decided,
not built).

## Related

- Scope: [`insight-assignee-notify-scope.md`](../../scope/insights/insight-assignee-notify-scope.md)
- Parent: [`insight-triage-scope.md`](../../scope/insights/insight-triage-scope.md) ·
  [`insight-triage-session.md`](insight-triage-session.md)
- The grammar extended: `scope/insights/insight-subscriptions-scope.md`; the ladder argued with:
  `scope/insights/insight-notify-scope.md`
- Skill doc: `docs/skills/insights/SKILL.md` · Public: `doc-site/content/public/insights/insights.md`
