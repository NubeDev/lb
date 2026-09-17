# Session — pinning a curated menu row (`nav:<navid>/<rowid>`)

Status: **tested** (2026-09-17). Scope: [`scope/nav/nav-row-pins-scope.md`](../../scope/nav/nav-row-pins-scope.md).
Branch: `feat/nav-row-pins`. Unreleased — needs the next `node-v*` tag before rubix-ai can bump to it.

## Why

A rubix-ai member browsing a curated site menu saw no pin affordance anywhere. The shell only draws a pin
on a row whose ref the resolver can round-trip, and a site menu is almost all rows with no such ref:
boards bound to variables (`Energy` bound to `site=chullora`) and folders that open their own board.
Pinning `dashboard:energy` would have opened the shared board with no site.

## What was built (`rust/crates/host/src/nav/`)

- `model.rs` — `NavItem.id` (serde-default, skipped when empty) + `MAX_ROW_ID_LEN = 64`.
- `row_ids.rs` (new) — `assign_row_ids`: keep a valid id, mint one for a row without, re-mint a
  duplicate after its first occurrence; `check_row_id` refuses a malformed id (`BadInput`).
- `save.rs` — `nav.save` runs `assign_row_ids` after the existing bounds check. The only writer of items.
- `resolved.rs` — `ResolvedItem` gains `id` (every row resolved from a stored item), and `nav_id` +
  `trail` (set only on a row pin).
- `resolve.rs` — `resolve_item` stamps `id` once, from the stored item, after the kind dispatch (so a
  tag-/template-group's generated children carry none). `item_ref` / `readable_nav` widened to
  `pub(super)` for the new resolver; the 8 explicit `ResolvedItem` literals gained the three fields.
- `resolve_row_pin.rs` (new) — the fifth pin grammar: read the nav through the pick tiers' readability
  gate, find the row by id (collecting folder labels), resolve it through `resolve_item`, pin a folder
  as its board (strip one with none), hide beats pin on the row's target ref. Every miss is `Ok(None)`.
- `resolve_pins.rs` — routes a `nav:` pin to `resolve_row_pin` first (its own prefix, no overlap).

No new caps, verbs or gateway routes; no migration (an old nav's rows gain ids on their next save).

## Tests (real `Node::boot()` / `Store::memory()`, no mocks)

`crates/host/tests/nav_row_pins_test.rs` (new, 4):
- `save_keeps_mints_and_reminted_duplicate_row_ids` — kept / minted / duplicate re-minted, re-save is
  stable, malformed id is `BadInput`.
- `resolve_echoes_row_ids_at_every_depth`.
- `row_pin_resolves_as_the_row_binding_label_and_trail` — bound board keeps vars/label/trail/nav_id; a
  folder pins flat as its board; a board-less folder strips.
- `row_pin_strips_silently_and_restores_free` — unreadable nav strips; readable nav + private board
  strips; shared board renders; hide strips, un-hide restores; deleted row + malformed refs strip; the
  stored pins are never mutated.

`nav_test::depth_at_cap_accepted_over_cap_rejected` compared a saved tree for equality; it now clears
the minted ids first (`clear_row_ids`) — the save legitimately adds them.

```
nav_row_pins_test           4 passed
nav_test                   50 passed
reusable_pages_test         8 passed
nav_home_test               8 passed
nav_footer_test             3 passed
nav_context_builtins_test   6 passed
nav_ext_boards_gate_test    4 passed
lb-host --lib nav          35 passed
lb-role-gateway nav_default_route_test 3 passed, nav_reach_test 2 passed
```

## Live check (rubix-ai against this branch via `cargo build --config patch…`, scratch store)

- `POST /navs` minted ids on every row; `GET /nav/resolve` echoed them at both depths.
- In the browser, pinning the vars-bound `Energy` row and the `Chullora` folder heading put
  **Chullora › Energy** and **Chullora** in Pinned; both survived reload and opened
  `dashboards/energy?var-site=chullora` / `dashboards/overview?var-site=chullora`.
- Saving the menu from the rubix-ai builder (title edit) kept every row id; both pins still resolved.

## Follow-ups

- Release: tag `node-v*`, then rubix-ai bumps the pin (its sidebar change is on
  `NubeIO/rubix-ai` `feat/pin-curated-nav-rows`).
- `resolve.rs` was already over the 400-line FILE-LAYOUT limit (552) and is now 577; splitting it is
  its own change.
