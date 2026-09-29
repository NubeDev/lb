# Insights scope — case self-clear: untouched work whose insights cleared closes itself

Status: **shipped** (2026-09-29). Session:
[`sessions/insights/case-self-clear-session.md`](../../sessions/insights/case-self-clear-session.md).
Extends [`case-plane-scope.md`](case-plane-scope.md) §"Reactors".

The case plane made the case the owner of triage (case-plane decision 5): a producer resolving its
insight does not close the case, because closing work is somebody's decision. That is right for work
a person is doing, and wrong for the far larger share nobody ever touched. On a metering estate a rule
flags a meter that went flat, the meter comes back the next day, the rule closes its insight — and the
case sits in `to_action` for ever. Found on a live node (2026-09-29): 393 of 1,182 open cases had
every insight already resolved. The queue counted them as open work, and an export of the insights
said "resolved" beside a case that said the opposite. Operators read that as the product disagreeing
with itself.

## Goals

- A **service policy can decide once** that a case whose insights all cleared, and that nobody has
  worked, is done — resolved `self_cleared` by the platform, not left for a person to click.
- **Opt-in** (`auto_close_self_cleared`, `false` by default): no workspace's behaviour changes until
  its policy says so.
- **Never closes worked cases.** Any sign a person engaged — assignee, waiting-on, snooze, a stage
  change, a comment (including one replayed from the insight), a hold-down reopen — keeps the case for
  a person.
- **Honest history.** The resolution is `self_cleared`, the actor `system:self-clear`, the event is the
  ordinary `resolved` event — a reader can tell exactly what happened and why.
- **Catches up by itself.** It runs on the existing case-reactor tick, so a workspace that turns the
  flag on has its backlog closed on the first pass.

## Non-goals

- **Not on the insight write path.** `lb_insights::resolve` stays a pure status write; nothing new
  subscribes to it. The pass derives the answer from the durable records each tick (the reconcile
  pattern), so a resolve that races a crash is still caught.
- **Not "who resolved the insight".** The resolve verb cannot tell a rule's `insight.close` from a
  person's `insight.resolve` (both run as a principal). This scope does not need to: if every insight
  is resolved and nobody worked the case, the case has nothing left to do either way. Triage lives on
  the case, so a person acting on the work acts on the case — which the "untouched" test sees.
- **Not a reopen.** A closed case is never touched. A problem that returns re-raises its insight and
  the grouping opens a NEW case; `self_cleared` is not held down (only `fixed` is).

## Intent / approach

One new reactor, `host/src/case/self_clear.rs::close_self_cleared_cases(node, ws, now)`, called on
the same tick as `reconcile_cases` and `backfill_case_facets` (`case/reactor.rs`), last so it judges
the cases those passes just opened or repaired.

Per tick, per workspace:

1. Read the policies; keep the ids with `auto_close_self_cleared`. **None ⇒ return, no estate scan.**
2. Scan the cases once; keep the open ones governed by an opted-in policy (`case.policy_id`, stamped
   by the SLA clock at open) whose own fields say "untouched": `workflow = to_action`, no
   `assigned_to`, no `waiting_on`, no `snooze_until`/`snoozed_by`, `reopened_count = 0`.
3. Scan the insights once (the resolved set) and the memberships once (case → insights). Keep a case
   only if **every** member insight is resolved. A case with no membership rows is judged by its
   primary.
4. Only for the survivors, read the history: every event's actor must be `system:`. One human event
   — a comment, an assignment, a replayed comment keeping its original author — and it stays.
5. `lb_cases::workflow(Resolved, SelfCleared, actor = system:self-clear, now)`. Per-case failures are
   logged and retried next tick; they never stop the pass.

Cost: three table scans and one policy read per tick when opted in; history reads only for the
candidates (after the backlog, a handful per tick). Not opted in: one policy read.

## How it fits the core

- **Rule 10:** nothing here knows a category, a site or a rule. The decision is data on the policy,
  the evidence is the case's own fields and history.
- **The case owns triage** is unchanged: the platform closes only what no person has taken up, and
  only where a policy — an admin's decision — says so.
- **Cap wall:** the reactor is a node-level pass like `reconcile_cases`, not a verb; it writes through
  `lb_cases::workflow`, the one implementation of the resolved-requires-resolution invariant.

## Configuration

```json
policy.sla.set { "id": "default", "match": {}, "respond_h": 4, "resolve_h": 24,
                 "auto_close_self_cleared": true }
```

Per policy, so a workspace can opt in for, say, metering categories and keep another category for
people. Absent on an existing row ⇒ `false`.

## Testing plan

Real booted node, `mem://` store, no mocks — `host/tests/case/reactor_self_clear.rs` in the
`case_suite` binary. The pass is a Rust function (no cap to remove), so each test asserts the case is
OPEN with its insight resolved **before** the pass runs (the reconcile precedent):

- untouched + opted in ⇒ resolved `self_cleared` by `system:self-clear`; a second pass closes 0;
- no policy, and a policy with the flag `false` ⇒ nothing closes;
- insight still firing ⇒ nothing closes;
- a comment / an assignment / a stage change ⇒ that case stays, while an untouched sibling closes (so
  the zero is the guard, not a dead pass);
- a verdict case of four members waits until the fourth is resolved;
- a returning problem opens a new case and the self-cleared one keeps its history.

End to end on a live node through the product surface: see the session log.

## Risks

- **A "touch" the pass cannot see.** A person who only *looked* at a case left no trace, so it can
  close under them. Acceptable: the insight already reads resolved, and the policy opted in.
- **Scan cost on very large workspaces.** Three scans a minute. Mitigated by the early return when no
  policy opts in; if it bites, the fix is a `status`/`closed` index, not a different design.

## Related

- [`case-plane-scope.md`](case-plane-scope.md) — decision 5 (the case owns triage), §Reactors.
- `host/src/case/hold_down.rs` — why only `fixed` reopens.
- rubix-ai: the Insights page counts case stages (PR #405), so this is what makes its "Resolved" tile
  and an insights CSV export agree.
