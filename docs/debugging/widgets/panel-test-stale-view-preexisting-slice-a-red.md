# `panel_test` "STALE" view — a pre-existing Slice A blast-radius (open)

- **Symptom:** `cargo test -p lb-host --test panel_test` has 4 failing cases:
  `ref_hydrates_coexists_propagates_and_ignores_echoed_spec`,
  `cross_ws_ref_rejected_and_dangling_placeholders`,
  `delete_refused_while_in_use_unless_forced`,
  `dashboard_save_returns_hydrated_ref_cells` — each panics with
  `BadInput("cell c1/c2: unknown view 'STALE' — call dashboard.catalog for the palette")`.
- **Area:** `widgets` (Slice A's `check_view_cells` validator × the `panel_test` fixtures).
- **Surfaced:** while running the broader suite during widget-platform Slice B (pin-to-dashboard),
  2026-07-04. NOT a Slice B regression — see "Is this mine?" below.
- **Status:** **open** (a named Slice A follow-up; not absorbed into Slice B — out of scope).

## Is this mine? (no)

`git diff --stat HEAD` for Slice B confirms its Rust changes are purely ADDITIVE — a new `pin.rs`, a new
`dashboard.pin` dispatch arm, re-exports, a gateway route, a member-cap line, and a descriptor row. Slice B
did NOT touch `dashboard/save.rs`, `dashboard/views.rs`, `dashboard/genui.rs`, `dashboard/bounds.rs`, or
`panel/validate.rs`. The `check_view_cells` call at `save.rs:48` is byte-identical to before Slice B.

`git log -- rust/crates/host/tests/panel_test.rs` shows the file was last touched at `de9e9a7 "added
backend widgets"` — the Slice A commit. So Slice A shipped these reds; they are pre-existing.

## Root cause

The `panel_test` fixtures construct a ref cell whose **echoed spec** (the spec a client sends back on
`dashboard.save`, which the host strips because the `panel_ref` is authoritative) carries `view: "STALE"`
as a placeholder — deliberately not the panel's real view, to prove `dashboard.get` hydration overwrites
the echoed spec with the panel's spec (the "ref is authoritative" rule, library-panels scope).

Slice A's `check_view_cells` validator (`dashboard/views.rs`) runs in `dashboard.save` BEFORE
`validate_and_strip_refs` (the ref-stripping step), so it sees the echoed `view:"STALE"` in `cells[]` and
rejects it — `"STALE"` is not a known built-in view, a well-formed `ext:` key, or `genui`. The save aborts
before the ref is stripped, so the test's `unwrap()` panics with the validator's `BadInput`.

The order in `save.rs`:
```
check_cells_bounds(&cells)?
check_genui_cells(&cells)?
check_view_cells(&cells)?           // ← rejects "STALE" here
validate_and_strip_refs(...)?       // ← would have stripped the echoed spec (never reached)
```

This is Slice A's documented blast-radius (widget-catalog scope, "the blast radius of validation:
`dashboard.save` validates the whole `cells[]`, so ONE unknown-view cell makes the ENTIRE dashboard
unsavable (even a title edit). That is acceptable in dev mode and it is the genui precedent's
behavior") — realized on the panel-test fixtures.

## Fix (the long-term-right call — for the Slice A owner / a named follow-up)

The fix is a TEST-FIXTURE change (no production code). The echoed-spec placeholder should use a REAL but
DIFFERENT view from the panel's real view — e.g. the panel's real spec is `view:"stat"`, the echoed spec
says `view:"gauge"`. Then:
- `check_view_cells` passes (both `stat` and `gauge` are known built-ins).
- `validate_and_strip_refs` strips the echoed `gauge` spec (the ref is authoritative).
- `dashboard.get` hydration re-expands from the panel → `view:"stat"`.
- The test's intent (the echoed spec is ignored, the panel's spec wins) is STILL proven — `gauge ≠ stat`,
  so hydration must have overwritten.

Concretely: in `rust/crates/host/tests/panel_test.rs`, find the `inline_cell` / ref-cell fixture that sets
`view: "STALE"` (lines ~103, ~105, and the `view` assertions at ~402, ~613 that read the hydrated
`view` — those should read the panel's real view, NOT "STALE"), and replace `"STALE"` with a real
built-in view that DIFFERS from the panel's real view (so the overwrite is observable). Then the 4 cases
pass and the "stale echoed spec is ignored" intent is preserved.

Why Slice B did NOT absorb this: (1) out of scope (Slice B = pin-to-dashboard; the panel fixtures are
library-panels territory); (2) the fix requires understanding the panel-test's "stale" intent to avoid
masking it (changing `"STALE"` → the panel's real view would make the test tautological — it would no
longer prove hydration overwrites); (3) silently absorbing Slice A's leftover work into Slice B would hide
the trail. Surfaced here per the debugging-history rule.

## Regression guard

The 4 `panel_test` cases themselves are the guard — once the fixtures use a real-but-different view, they
fail-before-the-fix is reverted (a regression to `"STALE"` re-triggers the validator rejection).

## Lesson

A new host-side validator that runs on the FULL `cells[]` (before any ref-stripping) will reject any
test-fixture placeholder view that isn't a real built-in. When shipping such a validator (Slice A's
`check_view_cells`), sweep the test corpus for placeholder views (`"STALE"`, sentinel strings) in fixtures
that pass through `dashboard.save` — the validator's "accept only known views" rule is correct, but the
fixtures must use real views (or a real-but-different view when the test's intent is "the echoed spec is
ignored").