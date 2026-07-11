# Session — nested nav groups (a menu tree: groups nest recursively, capped at depth 5)

Date: 2026-07-12 · Scope: [`scope/nav/nav-builder-scope.md`](../../scope/nav/nav-builder-scope.md)
(the nested-nav extension; downstream scope lives in the rubix-ai checkout at
`docs/scope/nav/nested-nav-scope.md`)

## The ask

Let a `group`'s `items[]` hold other `group`s, recursively, capped at **depth 5** (top-level list =
depth 1; a `group` at depth 5 may hold leaves but no further group). Leaf kinds unchanged, allowed at
any depth. Fully **additive**: the persistence shape was already a recursive nested array, so no new
table, cap, verb, or migration; a nav authored today (≤1 level) stays valid.

## STEP 0 — findings (this decided the size of the change)

The persisted shape (`NavItem.items: Vec<NavItem>`) was **already recursive** — the one-level rule was
enforced in exactly two places, both of which *deliberately refused to recurse*:

1. **`bounds.rs`** — `check_item(item, nested: bool)` rejected a `group` whose `nested` flag was set
   ("nav groups may not nest (one level only)"), and `count()` summed only one level of nesting.
2. **`resolve.rs::resolve_group`** — iterated children but `continue`d past any `child.kind == "group"`
   ("One level only … guard anyway (never recurse)"), and always emitted the group even when empty.

Everything else was already depth-agnostic: `strip_hidden` **already recursed** into groups (it just
never pruned an emptied one); the gateway route (`role/gateway/src/routes/nav.rs`) passes
`lb_host::NavItem` straight through (the recursive `items` field round-trips on the wire with no
gateway change); `tag-group`/`template-group` expansion resolve via their own resolvers and are
unaffected. `template-group` even already threads a `depth` param for re-entrancy.

So the real work was: (a) the save-time depth check, (b) removing the two one-level guards, (c) making
`resolve_group` recurse **and** prune empty groups post-order.

## What shipped (three changes)

**1. Depth cap at save (`bounds.rs`).** `check_item` now takes `depth: usize` (top-level list = depth
1). A `group` at `depth > MAX_GROUP_DEPTH` is rejected with `BadInput` naming the limit; nothing
persists (no silent flatten/truncate). `count()` now recurses over **every** depth, so the existing
100-node cap counts every node (groups included). The two checks fire **independently**: a
wide-but-shallow tree over 100 nodes fails on the node cap; a narrow-but-deep tree fails on the depth
cap.

**2. One-level restriction removed.** The `nested`-flag rejection is gone; `resolve_group` no longer
skips nested groups.

**3. Recursive resolve + empty-group prune (`resolve.rs`).** `resolve_group` recurses uniformly
through `resolve_item` (nested groups included) and prunes **post-order**: resolve a group's children
first, then drop the group (`None`) iff its resolved `items[]` is empty. A group with ≥1 surviving
descendant — even one several levels down — stays, because a deep survivor keeps every ancestor group
alive. `strip_hidden` (already recursive) got the same post-order prune, so a group emptied by the
hidden-set also vanishes. Net invariant: **a permitted user never sees a folder that expands to
nothing.**

The `5` is sourced from **one place**: `nav::model::MAX_GROUP_DEPTH`, re-exported on the lib API as
`NAV_MAX_GROUP_DEPTH` (next to `NAV_MAX_ITEMS` etc.) so a consumer UI can echo the limit rather than
hardcode it.

## Constraints held

- **Rule 10** — `ext` leaves stay opaque ids matched against `ext.list`; the recursion never branches
  on any named ext or on depth-keyed special cases. The added logic is pure structural recursion.
- **Additive** — no new cap/verb/table/migration; a pre-existing nav (flat or ≤1-level) validates and
  resolves unchanged; the wire shape is identical.
- **Rule 9** — every new test boots the real store/caps/node through the lib API with seeded real
  records; no mocks.
- **Rule 8** — changes land in the existing per-verb nav files; no new file needed.

## Files

- `nav/model.rs` — new `MAX_GROUP_DEPTH` const (+ doc-comment updates dropping "one level").
- `nav/bounds.rs` — recursive `count()`, depth-parameterized `check_item`, depth-cap rejection.
- `nav/resolve.rs` — recursive `resolve_group` with post-order empty-group prune; `strip_hidden` prune.
- `nav/mod.rs` + `lib.rs` — re-export `NAV_MAX_GROUP_DEPTH`.
- `tests/nav_test.rs` — 6 new tests (below); updated `over_cap_items_rejected` (one-level nesting is now
  *accepted*, not rejected).

## Tests (real infra, seeded records) — all green (30/30 in `nav_test`)

- `depth_at_cap_accepted_over_cap_rejected` — save accepts exactly depth-5, rejects depth-6 with
  `BadInput` naming the limit, persists nothing on reject; the accepted deep tree round-trips
  identically via `nav.get` (order + nesting).
- `node_cap_and_depth_cap_are_independent` — a shallow group over `MAX_ITEMS` leaves still `BadInput`s
  on the node cap.
- `deep_leaf_stripped_but_route_still_reachable` — a `rules` leaf nested 4 deep is stripped by
  `nav.resolve` for a caller lacking `rules.run`; with the cap it survives (nav never widens, at depth).
- `empty_group_pruned_but_deep_survivor_keeps_ancestors` — an all-stripped subtree disappears; a group
  whose only survivor is 3 levels down STAYS (proves post-order, not leaf-level).
- `hidden_and_tag_group_apply_at_depth` — a hidden ref inside a deep group strips at depth; a tag-group
  nested in a group still expands to a flat, cap-bounded dashboard list at its position.
- Existing isolation (`workspace_isolation`) and lens (`nav_never_widens_…`) tests still green.

## Release

Embed-seam tag cut so the downstream embedder (rubix-ai) can bump its `lb-node` pin. NOTE: the task
brief said "next patch after `node-v0.1.12`", but the repo's latest embed tag is already `node-v0.3.0`
(== HEAD before this branch); the brief's premise was stale. Cut **`node-v0.3.1`** as the correct next
patch. The `lb-node` crate `version` (0.1.9, which keys the core-skill boot seeder) is left unchanged —
the skill corpus didn't change; the git tag is the release marker downstream pins.
