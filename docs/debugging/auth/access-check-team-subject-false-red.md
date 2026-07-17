# `dashboard.access_check` reported a team-shared asset as "not shared" to that very team

**Date:** 2026-07-16 · **Area:** auth-caps (access preflight / gate-3 identity) · **Severity:** false
negative — the preflight lied about the correctly-shared case · **Found by:** the
`dashboard.share_closure` dual-consistency test (share-closure scope), while building the remediation
verb

## Symptom

`dashboard.access_check {dashboard, subject: "team:ops"}` — the **headline** use of the verb ("will
this page work for the ops team?", access-model scope) — reported every team-shared dependency **red**:

```
TEAM  subject (team:ops)          -> panel:done  ok=false  "not shared to the subject (private/unshared)"
USER  subject (user:bob, in ops)  -> panel:done  ok=true   "shared/visible to the subject"
```

Both verdicts are about **one share edge** (`panel:done -[share]-> team:ops`). They cannot both be
right. The `user:` answer was correct; the `team:` answer was a false red.

Nothing failed loudly. A preflight-for-a-team just claimed a correctly-assigned dashboard would not
render — the exact opposite of the false-*green* the scope worried about, and arguably the more
corrosive failure: it sends an admin to "fix" sharing that is already right, and it teaches them the
preflight is noise.

## Root cause

Gate 3 (`panel/visibility.rs::may_read_panel`, and identically
`dashboard/visibility.rs::may_read_dashboard`) resolves team access as:

> is this principal a **`member`** of a team the asset is **`share`d** to?

It walks `team -[member]-> user` edges looking for `principal.owner_sub()`.

`access_check` built its synthetic subject principal with the subject's own key as the sub:

```rust
let subject_principal = Principal::routed(subject.as_key(), ws, subject_caps);
//                                        ^^^^^^^^^^^^^^^^ "team:ops" for a Team subject
```

So for a `team:ops` subject, gate 3 asked: **is `team:ops` a member of `team:ops`?** A team is not a
member of itself, so the walk never matched and the team branch fell through to `Denied` — for every
team-shared asset, no matter how correctly it was shared.

The caps half of the synthetic principal was right (`resolve_subject_caps_live` folds a team's grants
properly), which is why this was invisible in the cap-gap verdicts and only ever wrong on the gate-3
share verdicts.

**Why it shipped:** every existing test in `dashboard_access_check_test.rs` preflights a `user:`
subject, for which `subject.as_key()` IS the right gate-3 identity. The `team:` path — the one the
parent scope calls the headline — had no test at all. The module's own doc even advertises
`subject/team` and the report's `subject` field documents `user:bob / team:ops`.

## The fix

`access_check::gate3_identity` — for a `Team` subject, ask gate 3 as a **representative member** of
that team:

```rust
let subject_principal =
    Principal::routed(gate3_identity(store, ws, subject).await?, ws, subject_caps);
```

"Can team ops read this?" *means* "can ops's members read this?", so asking as a member is the same
question, not an approximation.

Deliberately **not** the alternative: teaching `may_read_panel`/`may_read_dashboard` to special-case a
`team:` principal. That would fork the visibility rule into a preflight variant and a live variant —
the "preflight/live divergence is the cardinal sin" the module doc opens with. One gate-3, asked
honestly, is the whole design.

An **empty** team keeps its own key and so resolves red. That is correct, not a fallback: a team with
no members grants no one access.

## Guard

- `dashboard_access_check_test::a_team_subject_resolves_assets_shared_to_that_team` — a panel AND a
  dashboard shared to `team:ops` are green for the `team:ops` subject; the team subject's verdict
  **equals** a real member's; and a team the asset is *not* shared to stays red (the fix greens only
  the real share, never blanket-greens a team subject). Verified RED on the pre-fix code with the
  exact symptom above.
- `dashboard_share_closure_test::share_closure_gaps_match_access_check_red_panels` — the
  dual-consistency test that caught it: the remediation's gap set must equal the detection's red panel
  set. Two verbs answering the same question about one share edge cannot disagree. This class of bug
  is invisible to a test that only ever asks one of them.

## Lesson

The bug is a **subject-kind** bug, not a sharing bug: a `Subject` enum with four variants was fed to a
predicate that only understands one of them (`user:`), and the type system was happy because both sides
speak `String`. `Subject::as_key()` is an identity for the *grant* store; it is not automatically an
identity for the *membership* graph, and the code read as if it were.

Two morals worth keeping:

1. **When a verb advertises N subject kinds, test N subject kinds.** The `team:` path was documented,
   dispatched (`tool.rs` parses `subject`/`team` args), and shipped — with zero coverage. "It takes a
   `Subject`" is not the same as "it works for every `Subject`".
2. **A dual is a test oracle.** This survived its own test suite and was caught the moment a second
   verb had to agree with it about the same fact. Where two verbs read one truth, pin them to each
   other — the agreement test finds what neither verb's own tests can see.
