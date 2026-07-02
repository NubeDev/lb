# Adding an "AI widget" is rejected `options.genui is missing` / renders `invalid genui widget (no IR)`

- Area: frontend
- Status: resolved
- First seen: 2026-07-03
- Resolved: 2026-07-03
- Session: ../../sessions/genui/genui-widget-session.md
- Regression test: rust/crates/host/tests/dashboard_genui_test.rs (`allows_an_unauthored_draft`),
  ui/src/features/dashboard/views/genui/GenUiView.test.tsx (draft/invalid classification),
  ui/src/features/dashboard/views/genui/genui.gateway.test.tsx (`SAVES an un-authored draft genui cell`)

## Symptom

Picking **"AI widget"** in the viz picker and saving (before generating anything) failed with the
host error `cell w2 (genui): view is "genui" but options.genui is missing`, and the panel rendered
`invalid genui widget (no IR)`. So a genui widget could not be *added* the normal way — you'd have to
somehow author an IR before the cell could exist.

## Root cause

A freshly-picked genui cell has **no IR yet** — `defaultOptionsForView("genui")` returns `{}` (the
author generates the IR in the "AI widget" tab afterwards). But both the host and the view treated
"no IR" as **malformed**, not as an un-authored draft:

- `dashboard/genui.rs::check_genui_cell` did `options.get("genui").ok_or_else(… "missing")?` — a
  missing block was a hard `BadInput`. So `dashboard.save` refused the cell.
- `GenUiView.cellIr` returned `null` for both "no block" and "broken IR", and the view rendered the
  same `invalid genui widget` message for both.

This is the wrong model: adding a blank widget you configure later is normal (you add a blank
timeseries, then bind it). An empty genui cell is a **draft**, not an error.

## Fix

Treat "no IR authored yet" as a legitimate savable draft; only reject a *present-but-malformed* IR.

- Host (`genui.rs`): a missing `genui` block, or a block with no `ir` (or `ir: null`), returns
  `Ok(())` (a savable draft). A present `ir` that is **not an object** is `BadInput("… must be an
  object")`. Everything else (v known, ≤8 KB, catalog names, root) validates as before, only once an
  actual IR is present.
- View (`GenUiView.cellIr`): returns `"draft" | "invalid" | { ir }`. A draft renders a muted
  "AI widget — open the editor's 'AI widget' tab and describe it to generate." placeholder; a
  present-but-broken IR renders `invalid genui widget (malformed IR)`; a good IR renders the surface.

## Regression guard

- Host `allows_an_unauthored_draft`: (a) no `genui` block and (b) a block with no `ir` both save;
  (c) a non-object `ir` is rejected `must be an object`.
- `GenUiView.test.tsx`: draft → author-me placeholder (not "invalid"); malformed → "invalid".
- Gateway `SAVES an un-authored draft genui cell`: the exact reported flow (add → save → render) over
  the real node.

## Lesson

A "configure it later" widget must be creatable EMPTY. Validation that assumes the artifact is always
fully-authored blocks the add flow — distinguish an **un-authored draft** (allowed, guided) from a
**malformed spec** (rejected) at both the write boundary and the view.
