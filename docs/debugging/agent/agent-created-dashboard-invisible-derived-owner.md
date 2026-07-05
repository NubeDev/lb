# Agent "successfully" saved a dashboard the user could not see — owner was the derived `agent:session` sub

**Area:** agent (delegation) + dashboard ownership
**Date:** 2026-07-05
**Symptom:** The widget-builder run answered "dashboard created!" and `dashboard.save` really had
succeeded — but the user's dashboard list didn't show it, and `dashboard.get` on its id returned
the opaque `denied`. The model had even tried `dashboard.share` to work around it (which failed on
an arg error), hinting it "knew" the record wasn't visible.

## Root cause

The agent loop runs tools under the **derived** principal (`agent:session`, `agent ∩ caller` caps —
correct for audit and authorization). But `dashboard_save` stamped `owner = principal.sub()` =
`agent:session`, and create defaults `visibility = private`. A private record owned by a sub nobody
logs in as is invisible to everyone — including the human the agent acted for.

## Fix

`Principal` now records the delegation root: `derive()` sets `delegator` to the ORIGINAL caller's
sub (preserved across nested derives, same rule as `constraint`), and `owner_sub()` returns
`delegator || sub`. Ownership stamps and owner/visibility walls in the dashboard + panel verbs
(`save`/`share`/`delete`/`pin`/`may_read_*`) read `owner_sub()` — so a record the agent creates on
your behalf belongs to YOU, while the audit trail still shows the `agent:*` sub acted. Capability
checks are untouched (still `agent ∩ caller`).

Note: a record created BEFORE this fix (dev store: dashboard `meter-usage` owned by
`agent:session`) stays orphaned — no principal's `owner_sub` is `agent:session`. Dev-store cleanup;
not migrated.

## Regression tests

`lb-auth` unit tests + the dashboard/panel suites (`dashboard_test`, `panel_test`,
`dashboard_genui_test`) run green over the `owner_sub` switch; the mandatory deny/isolation tests
still pass (the wall is unchanged — only ownership resolution moved).

**Verified live:** the retest run saved a new cell into the user's open dashboard (`keep-dash`,
owner `user:ada`) and the user's `dashboard.get` returned it with the new cell hydrated.
