# The push target resolved every effect's devices in a hardcoded `"acme"` workspace

- Area: inbox-outbox (push target)
- Status: fixed
- First seen: 2026-07-11 (peer review of the push-target slice)
- Resolved: 2026-07-11
- Session: ../../sessions/inbox-outbox/push-target-review-fixes-session.md
- Regression tests: `rust/crates/host/tests/push_deliver_test.rs`
  (`effect_missing_workspace_fails_instead_of_guessing`, `non_member_audience_sub_is_excluded`,
  plus every deliver-path test now runs against a payload-carried workspace)

## Symptom

`notify/push_target.rs::effect_workspace()` literally returned `"acme".to_string()` — under a
wall of comments describing three different designs and implementing none. Every push effect,
whatever workspace it was enqueued in, resolved devices / prefs / disable-writes in `acme`:
a ws-B notification would fan out to ws-`acme` devices (rule 6 violation, the hard wall),
and in any workspace not named `acme` push silently did nothing.

It shipped green because `deliver()` had **zero tests** — the only "test" surface was the
recording fake in isolation.

## Root cause

`Target::deliver(&self, effect)` receives no `ws` parameter (the relay loop holds it), and the
slice stopped at the seam instead of closing it: the enqueue side never embedded the workspace,
so the deliver side had nothing real to read and a test-convenient constant was left in.

## Fix

The email-target pattern, applied symmetrically:

- `notify/verbs.rs::notify_send` embeds `"workspace": ws` in the effect payload at enqueue time
  (the ws is already authorized there — `authorize_tool(principal, ws, …)` ran first).
- `PushPayload` gained `workspace: Option<String>`; `deliver()` reads it and **fails the effect**
  (`Err`, so the outbox retries → dead-letters) if it is absent or empty — never guess a
  workspace. `effect_workspace()` and its self-contradicting comment wall are deleted.
- While here: the audience is now membership-checked (`lb_authz::membership_is_member`) in that
  ws — a `sub` outside it is silently excluded (the scope's mandatory isolation case).

## Lesson

A `Target` is only as workspace-scoped as the data it is handed: when a trait seam drops the
tenancy parameter, close it by carrying `ws` **in the payload written by the authorized verb**
— and make the deliver side fail loudly without it. A hardcoded tenant "for v1 tests" plus zero
deliver tests is exactly how a rule-6 hole ships green.
