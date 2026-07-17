# `dashboard.share_closure` reported "shared" while the widget stayed "Panel not accessible"

**Date:** 2026-07-16 · **Area:** dashboard (share_closure / S4 edge identity) · **Severity:** the verb
was inert — it reported success and changed nothing the viewer could see · **Found by:** the user, in
the browser, immediately after the feature "passed" 15 unit/integration tests

## Symptom

`user:bob` opens a team-shared dashboard. The page renders; the widget shows **"Panel not accessible —
This library panel was removed or isn't shared with you"**. The page owner runs the new remediation
verb, which reports complete success:

```json
{"dashboard":"ops-page","team":"team:ops","dry_run":false,
 "panels":[{"panel":"panel:panel-1784200360179","disposition":"shared","reason":"shared with team:ops"}]}
```

The panel record even flips to `visibility: "team"`. And bob's page is **unchanged** — still
`panelMissing: true`, still the placeholder. `panel.get` as bob: `denied`.

The verb lied. Every one of its 15 tests passed.

## Root cause

A team has **two different identities** in this codebase, and the verb picked the wrong one:

| identity | where it is the truth | example |
|---|---|---|
| **bare** (`ops`) | the S4 relation graph — `member` and `share` edges; `teams.list` output | `member__ops__user:bob` |
| **`team:`-prefixed** (`team:ops`) | the GRANT store — `Subject::Team("ops").as_key()` | `grant(team:ops -> cap)` |

`share_closure::normalize_team` "helpfully" normalized every input to the **grant** form and shared to
that. The live edges tell the whole story:

```text
member__ops__user:bob                     <- bob's membership: BARE
share__ops-page__ops                      <- dashboard.share's edge: BARE   => bob reads the page
share__panel-1784200360179__team:ops      <- share_closure's edge: PREFIXED => dead end
```

Gate 3 (`may_read_panel`) resolves in two hops: *teams this panel is shared to* → *members of that
team*. Hop 1 returned `["team:ops"]`. Hop 2 asked for members of `team:ops` — and **nothing is a member
of `team:ops`**; the members live under `ops`. So the walk found no one, denied bob, and the widget
stayed dead while the panel sat there marked `visibility: team` with a share edge pointing into
nowhere.

Every other verb passes `team` through **verbatim** (`add_member`, `dashboard_share`, `panel_share` all
just `relate(store, ws, KIND, a, team)`). The platform is prefix-agnostic and the live system is
consistently bare. `share_closure` was the only verb that rewrote the caller's team id — and rewriting
it is what broke it.

The same defect existed in `access_check::gate3_identity` (the fix for
`access-check-team-subject-false-red.md`): it looked up the team's members under
`Subject::as_key()` → `team:ops`, found none, and silently fell back to the team's own key — i.e.
straight back into the false-red it was written to fix.

## Why 15 tests missed it

**The tests were self-consistent and reality was not.** They seeded membership with
`add_member(…, "team:ops", "user:bob")` and shared to `"team:ops"` — both sides of the same wrong
convention, so gate 3's two hops matched each other and the suite went green. The fixture agreed with
the fixture. Nothing in the suite had ever seen a real `member` edge.

The dual-consistency test — the one designed to catch exactly this class — was fooled for the same
reason: **both** verbs were consistently wrong in the same direction, so they agreed.

This is the sharpest lesson available about test-fixture realism: a test that constructs its own world
can only ever check internal consistency. It cannot tell you the world is shaped differently. The bug
was found in a browser in about 90 seconds.

## The fix

- `share_closure::normalize_team` → normalize to the **bare** id (accept `ops` and `team:ops`, unwrap
  the latter), so the edge writers stay verbatim as they are for every other verb.
- `access_check::gate3_identity` + `share_closure::team_member_probe` → look members up under the bare
  name first, tolerating the `team:` form as a fallback (both shapes exist in the wild).
- The **tests** were re-seeded to use bare ids for membership and edges — matching what `members.add`
  actually writes — while still passing `team:ops` as the verb's *argument*, which proves both input
  forms normalize correctly.

Verified against the running node and in the browser as `bob@acme.com`: the widget renders its live
table (`site-001 Northside Factory`, …) instead of the placeholder.

## Lesson

Two lessons, and the second is the one that matters.

1. **A "helpful" normalization is a lie about identity.** Every other verb treats `team` as an opaque
   string, so the callers and the graph already agreed. `normalize_team` invented a canonical form that
   nothing else in the system used, and the type system was delighted: both are `String`. Where an id
   has two forms in one codebase, the verb that rewrites it is the one that breaks.
2. **Passing tests are evidence about the tests, not about the system.** 15 tests, a deliberate
   adversarial RED proof for each, a cross-verb agreement test — and the feature was inert on the first
   real page load. All of it validated a fixture world that never existed. **Drive the real thing
   before claiming it works.** The unit tests were still worth writing (they caught two genuine bugs),
   but they can only ever prove the code does what the fixture says. The browser proves it does what
   the user needs.
