# A scheduled rule was `denied` on every fire — `reactor_caps()` never granted the verbs a rule uses

- **Area:** rules / flows
- **Symptom:** A rule with a `#[schedule(...)]` directive saved correctly, compiled to a managed
  `cron → rule` flow, and the cron **fired exactly on time** — then the `rule` node was `denied` on
  every single fire. Every run settled `partialFailure` and did nothing. `rules.run` on the same rule
  succeeded, which is why it never showed up in manual testing.
- **Status:** resolved
- **Date:** 2026-08-14 (issue [lb#167](https://github.com/NubeDev/lb/issues/167); first half fixed in
  [#168](https://github.com/NubeDev/lb/pull/168), completed here)

## What was observed

On a live node:

```
call flows.runs.list '{"flow_id":"schedule:cron-probe"}'
# → [{"runId":"schedule:cron-probe-cron-trigger-1786663800","status":"partialFailure"}]

call flows.runs.get '{"run_id":"…"}' | jq -c '.steps'
# → [{"id":"rule","outcome":"err","error":"denied"},
#    {"id":"trigger","outcome":"ok","output":{"payload":{"cron_ts":1786663800}}}]
```

Trigger `ok`, rule `denied` — six consecutive fires, all identical.

## Root cause

A scheduled fire runs under the fixed system principal `node:reactor`, minted by
`spawn_flow_reactors` from `reactor_caps()` (`crates/host/src/flows/reactor_loop.rs`). A MANUAL run
rides the **caller's** token, which is why the same rule worked by hand. The two authorities are not
the same list, and the reactor's was missing every verb a rule actually finishes with:

1. **`mcp:rules.eval:call`** — the `rule` node's own dispatch (`flows/src/builtins/core.rs`,
   `HOST_RULES_EVAL`). Without it nothing in the rule ever runs. Note `mcp:*.call:call`, already in
   the list, does **not** cover it: the mcp resource splits on `.`, so that pattern means "second
   segment is literally `call`", and `rules.eval` is not.
2. **`mcp:insight.raise:call`** (+ `ack`/`resolve`) — a rule that raises is the ordinary FDD/EMS
   shape, and raising carries a lifecycle, so granting `raise` without the ageing pair is a
   half-grant.
3. **`mcp:inbox.record:call` + `mcp:outbox.enqueue:call`** — the *other* finishing move,
   `alert(...)`. The host fans every alert-marked finding out to inbox + outbox at the end of a
   successful eval (`rules::run::route_alerts`) under the calling principal; a deny there fails the
   whole `rules.eval`. So even with (1) and (2) granted, an alerting rule stayed `denied` — the same
   asymmetry, one verb deeper.

The function's own comment already described this exact failure mode for the `ext-list`/`store-*`
nodes and fixed it for them:

> Without them a scheduled flow's `ext-list`/`store-*` node is `denied` while a MANUAL run (the
> user's token) succeeds — the asymmetry that reads as `partialFailure`.

The rule node was simply never added to the same list.

### Why a green suite coexisted with a permanently broken product

`scheduled_rules_test.rs` drove the managed flow with the author's **FULL** test principal, which of
course holds `mcp:rules.eval:call`. It exercised the reactor's *code path* while substituting a
different *authority* — the one variable the bug lived in. The suite was green through six failing
fires on a live node.

## Fix

- `reactor_caps()` grants the named verbs above. Named, not blanket: `mcp:*.*:call` was rejected for
  `ext-call` for good reason and stays rejected. All of these are workspace-scoped durable-write
  verbs the reactor already holds the store surface for; none reaches a third party (an outbox
  effect is *enqueued* — delivering it is the outbox worker's own principal's job).
- `reactor_caps()` is `pub` so a test mints the **production** principal from the function itself. A
  hand-copied cap list in a test silently stops testing the function the moment the two drift, which
  is precisely how this shipped green.

### Related, fixed with it: `rules.delete` orphaned the managed cron

`rules.delete` tombstoned only the rule record, leaving `flow:{ws}:schedule:{id}` behind — a cron
firing forever at a rule that no longer exists, with no owner left to remove it through the rules
surface. `rules.delete` now runs the **same** teardown a re-save without the directive runs
(`schedule::sync_schedule(.., None)`), so there is one derived-state reconciler rather than two: it
is idempotent (an absent flow is a no-op, which also self-heals an already-orphaned one), and a
caller without flow-write gets the honest `pending` marker instead of a failed delete, exactly as on
save. The delete response now carries the `schedule` block.

## Regression

All in `crates/host/tests/scheduled_rules_test.rs`, all built on
`Principal::routed("node:reactor", ws, reactor_caps())` — **the real production principal, imported,
never mirrored**:

- `scheduled_rule_fires_under_the_reactors_own_principal_not_the_authors` — fires the managed flow
  under the reactor, asserts the rule step is not `denied`, the run is `success` (not
  `partialFailure`), and the insight actually landed.
- `scheduled_alerting_rule_routes_to_inbox_under_the_reactors_own_principal` — same, for an
  `alert()` body: asserts the routed inbox item on the `rules` channel.
- `rules_delete_tears_down_the_managed_flow` — after the delete, `flows.get` is gone AND a
  far-future reactor tick fires **nothing** (the effect, not just the record); a repeat delete is a
  no-op.

Revert-checked: with the caps removed the alerting test fails on `left: "denied"`; with the teardown
removed the delete test fails on the surviving flow.

## Lesson

**A headless actor's authority is a separate thing from the code path it drives, and only one of
them is usually under test.** Any test that proves a system principal can do its job must mint that
principal from the production function — a hand-copied list is a mirror that goes stale silently and
turns the test green precisely when the product breaks. And when a fixed system principal gains one
verb, ask what that verb *finishes with*: `rules.eval` alone left the alerting half of the same bug
alive, because the deny had moved one call deeper.

---

## Layer 3 (2026-08-18) — the deny the caps list could not fix

Reported still-broken on a live node: `schedule:pdnsw-aq-device-frozen` running every 30 minutes,
trigger `ok`, `rule` node `{"lastError":"denied"}` — with **both** fixes above shipped and the pin on
`node-v0.23.0`. The binary genuinely contained the grants (verified with `strings` on the running
`target/debug/rubix-ai`), so the caps list was not the problem this time.

The rule's body opens:

```rhai
let rows = query("pdnsw", ` ... `).records();
```

`query(<source>, sql)` resolves through the rule data seam and, for a **datasource** source, collects
via `federation.query` (`crates/host/src/rules/seam.rs::collect_federation`). That needs
`mcp:federation.query:call`, which `reactor_caps()` does not grant. Third layer, same asymmetry:
`rules.run` from the Rules page works (the user's token holds it), every scheduled fire is `denied`.

**Why layer 3 was not another `.into()` line.** Granting `federation.query` to `reactor_caps()` would
give every scheduled flow on the node blanket read access to every registered datasource, forever,
with no relationship to who wrote the rule — the same blanket-third-party-reach argument the
function's own comment already refuses for `ext-call`. Three layers in, the list was being extended
one denial at a time toward exactly the omnipotent system principal it was written to avoid.

**Fix: run-as-owner** (`crates/host/src/flows/execute_node/run_as_owner.rs`), the follow-up
`ext-store-nodes-scope.md` §348 had reserved. `rules.save` records `SavedRule::scheduled_by` (only
when the body carries a directive); a HEADLESS `rule` node substitutes a principal for that owner,
built from caps **re-resolved live** from the grant store on every fire. Owner is an identity, never a
stored credential; workspace-pinned; never wider than what the owner could already do by hand; and
every failure path (no owner, unresolvable, store error) degrades to the reactor principal, so a miss
is the old denial and never a widening. Manual runs are untouched — the swap keys on the
`node:reactor` sub.

Two traps hit while building it, both nearly invisible:

1. **The `user:` prefix.** Grants are stored under the BARE user name; `principal.sub()` is
   `user:test`. Passing the prefixed form to `resolve_caps_live` returns zero caps, which this code
   reads as "revoked owner" and degrades to the reactor — i.e. it fails exactly like the bug it
   fixes. Same prefix bug as lb#176's reminder fire path.
2. **The test passed with the fix deleted.** A first regression test *did* use the real reactor
   principal — the lesson from layer 2 — and still proved nothing, because its rule body only called
   `insight.raise`, a verb the reactor already holds. Every verb a rule can reach without a live
   datasource sidecar is one `reactor_caps()` grants.

The shipped regression
(`the_rule_node_dispatches_under_the_owner_not_the_principal_it_was_handed`) fires under a reactor
principal with `mcp:rules.eval:call` **deliberately stripped**, so the run can only succeed if the
node dispatched under the owner. Verified red before green.

**Migration.** Rules saved before this have no `scheduled_by` and stay broken until claimed. New verb
`rules.adopt_schedule {id}` (gate-aliased to `mcp:rules.save:call` — a bespoke
`mcp:rules.adopt_schedule:call` exists in no role bundle and would be unreachable for everyone,
including admins) records the caller as owner; re-saving the rule does the same thing. There is
deliberately **no silent backfill**: stamping a guessed subject onto every ownerless rule is a
privilege grant performed by a migration rather than by a person, and nothing in the store records
who authored a rule before the field existed.

## Lesson (layer 3)

**When a fixed system principal needs a third verb, the answer is probably not the third verb.** Two
grants were right — `rules.eval` and the alert pair are the mechanics of running a rule at all. The
third was a *data-reach* cap, and data reach belongs to a person, not to the node. The tell is that
the cap would have been granted to every scheduled flow rather than to this one: at that point the
question stops being "what may the reactor do" and becomes "who is this run acting for".
