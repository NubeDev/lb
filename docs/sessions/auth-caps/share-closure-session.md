# Share the dashboard's closure — session

- Date: 2026-07-16
- Scope: `docs/scope/auth-caps/share-closure-scope.md`
- Status: done (lb side; the guided UI is rubix-ai's, not built here)

## Goal

Close the live gap: a member opens a team-shared dashboard, the PAGE renders, and every embedded
library panel that is still `private` shows "Panel not accessible — isn't shared with you".
`dashboard.access_check` already **detected** that gap; the missing half was the **remediation**.

Build `dashboard.share_closure(dashboard, team, dry_run)` — the write dual of `access_check`: it shares
every library panel in a page's closure to a team, **only** the panels the caller owns, never
force-sharing one they don't. Explicitly NOT auto-share-on-embed: that would silently widen a panel's
audience by reference (the class of `debugging/auth/member-wildcard-satisfies-admin-cap.md`).

Session ran Phase 1 (scope review) then Phase 2 (implement) in one pass.

## What changed

### Phase 1 — the scope, amended before any code

The scope was sound in shape (dry_run-defaulted, owner-gated per panel, distinct from
`dashboard.share`) and I confirmed all three. Both open questions were resolved and recorded:
**explicit `team` in v1** (the whole-page-audience convenience is a different verb-shape, deferred),
and **`no_share_cap` distinct from `not_owned`** (different gaps, different human fixes).

Five things the scope missed, folded in before coding — the load-bearing one being
`Visibility::Workspace` panels (a fourth non-gap disposition; without it the UI nags to "fix" panels
that need nothing). Also: nested-panel `unchecked` cutting both ways, no `unshare_closure` in v1, the
report's deliberate shape-divergence from `AccessReport`, and the bulk-specific team-existence check.

**The one real inaccuracy corrected:** the scope claimed `share_closure` "reuses access_check's closure
walk". It cannot — `check_cell`/`check_target` are private and compute verdicts against the *subject's*
synthetic principal ("can the SUBJECT read this?"), while share_closure needs the *caller*-side question
("can YOU close this gap?"). Two principals, two questions. The scope now says what is actually
implementable: **extract** the enumeration, **call** `may_read_panel` + `panel_share` verbatim.

### `lb-host` — `dashboard/closure.rs` (new)

The **one** panel-closure enumeration (`closure_panels`), called by BOTH `access_check` and
`share_closure`. Two independent walks could drift about what the closure even IS — a share_closure
seeing fewer panels than access_check would report "all shared" while one stayed private (rule 9: no
parallel walk). De-dupes per panel id (one asset, one audience → one row). 5 unit tests.

### `lb-host` — `dashboard/share_closure.rs` (new)

The verb + `ShareClosureReport`/`ShareClosureItem`/`Disposition`. Six dispositions:
`would_share`/`shared`, `already_shared`, `already_visible_workspace`, `not_owned`, `no_share_cap`,
`unchecked`. Key properties:

- **Never touches `relate()`** — every share goes through `panel_share`, which re-runs the
  `mcp:panel.share:call` gate AND the owner rule. The verb structurally cannot widen anything
  `panel.share` would refuse (proven below).
- **Gate-3 by probe, not by re-implementation** — `team_can_read` asks the live `may_read_panel` as a
  **non-owner** member of the team (skipping the owner is load-bearing; see Debugging #2).
- **Team must exist in-workspace before ANY write** — a bulk verb must not scatter dangling edges
  across every owned panel off one typo'd/foreign team name. Refuses the whole call, no partial apply.
- `dry_run` defaults to **true** at the MCP boundary: an omitted, null, or non-boolean `dry_run`
  previews. Only an explicit `false` mutates.

### `lb-host` — `dashboard/access_check.rs` (refactor + a bug fix)

Panel enumeration now comes from `closure_panels`; `check_cell` handles inline sources only.
**Plus a real bug fix** — `gate3_identity` (see Debugging #1).

### `lb-host` — cap + wiring

`mcp:dashboard.share_closure:call` added to `AUTHOR_CAPS` by name (not a wildcard — the module's
exhaustive-by-name rule). No gateway change needed: `tool_call.rs` already prefix-routes `dashboard.*`
into `call_dashboard_tool`, so a new match arm is the whole wiring.

## Decisions & alternatives

- **Author tier, not admin.** It is a member's authoring action on their OWN panels; the per-panel
  owner rule is the real wall, so the cap cannot widen reach beyond panels the caller already owns. A
  viewer holds no `panel.share`, so the verb would be a pure no-op for them → not viewer either.
- **Panel-centric report, NOT `AccessReport`.** "What would this write do?" ≠ "will this page render?".
  Bridged to access_check by the **dual-consistency test**, not a shared struct — the honest coupling.
- **Rejected: calling `dashboard_access_check` and diffing its report.** Wrong principal, wrong
  question (see Phase 1 above). Rejected: a second closure walk (rule 9). Rejected: teaching
  `may_read_panel` about `team:` principals (would fork the visibility rule into preflight vs. live —
  the cardinal sin the module doc opens with).

## Tests

All RED-verified on pre-fix code first, then green. `cargo fmt --all --check` clean; my files
clippy-clean (the 21 remaining warnings are pre-existing, in `lb-viz`/`lb-mcp`/`lb-frame`).

`tests/dashboard_share_closure_test.rs` (11, real store, no mocks):
the live repro end-to-end (preview → confirm → the team member renders the widget, a non-member still
403s — the wall moved for `ops` only); **no-widening** (the load-bearing one); capability-deny;
workspace-isolation (ws-B caller; + the bulk edge: a non-existent team refuses the whole call with no
partial writes); workspace-visible panels are not gaps; `no_share_cap`; page-visibility gates the
closure but panel-ownership gates each share; idempotency + incremental; a dangling ref is reported not
dropped; **dual consistency vs. access_check**.

`tests/dashboard_access_check_test.rs` (+1): the team-subject regression (Debugging #1).
`authz/builtin_roles.rs` (+2): the cap's tier, by name and through the real matcher.
`dashboard/closure.rs` (+5): the enumeration.

**RED proofs run:**
- Removed `gate3_identity` → team-subject test failed with the exact symptom ("not shared to the
  subject (private/unshared)").
- Removed the cap from `AUTHOR_CAPS` → both tier tests failed.
- **Removed the ownership check from the disposition logic** → the no-widening test failed... with
  `Denied`, because `panel_share`'s own owner rule refused the write. The report lied; the wall held.
  That is the defense-in-depth the scope's "never reimplement the write" rule buys, demonstrated.

**Pre-existing failures, NOT mine:** `agent_persona_catalog_test` (6) fails identically on baseline
with this work stashed — a persona/skill-grant issue, untouched here.

## Debugging

1. **`access_check` team-subject false-red** — found by the dual-consistency test, fixed here, written
   up in `debugging/auth/access-check-team-subject-false-red.md`. Preflighting for `team:ops` reported
   assets shared to `team:ops` as red, because gate 3 was asked "is `team:ops` a member of `team:ops`?".
   Shipped untested: every existing test used a `user:` subject. **The scope's central bet paid off** —
   two verbs forced to agree about one share edge found what neither's own tests could see.
2. **The owner-masking probe** — my first `team_can_read` picked an arbitrary team member, which could
   be the panel's owner, whose owner-path `Ok` masks a real gap as `already_shared`. Caught by
   `page_visibility_gates_the_closure…` (owner is himself in the target team). Probe is now non-owner.
3. **`Store::open("mem://")` is not an in-memory store** — `open` hands its arg to SurrealKV as a
   filesystem PATH, so `mem://` is a shared on-disk dir (`crates/host/mem:/`, untracked, not
   gitignored); 11 parallel tests sharing it corrupted its manifest. `Store::memory()` is the real
   in-memory constructor — 48 of 49 host test files already use it. Mine now do too. (The one file
   still using the path form is left alone — out of scope.)
4. **`lb_host::add_member` is the ASSETS one**, gated by `store:doc/*:write`, not `mcp:members.add:call`
   — a test-fixture trap worth knowing.

## Follow-ups (not done here, deliberately)

- **The guided UI is rubix-ai's** (`scope/frontend/dashboard/share-closure-ui-scope.md`) + the
  `lb-node` tag bump. Not touched, per the session's lb-only remit.
- **Skill doc** — the scope now requires a `share_closure` section in `skills/dashboard-mcp/SKILL.md`
  (the dry_run→confirm two-step is exactly what a model gets wrong without a written how-to). Should be
  written against a live run, which this session did not have.
- **`share_closure` with no team → the page's whole audience** (resolved decision 1: deferred).
- **`may_read_dashboard` has the identical team-subject shape** as the panel bug; `gate3_identity`
  fixes both call sites at once (both are covered by the regression test), but any FUTURE caller that
  builds a synthetic principal from a `Subject` can re-introduce it. A `Subject`→gate-3-identity seam
  in one place would close the class.
