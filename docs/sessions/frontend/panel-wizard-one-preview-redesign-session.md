# Panel wizard — Options step redesigned to ONE pinned preview (2026-07-07)

## The ask

The shipped OptionsStep rendered one `OptionSectionCard` per registered option, and **each card
mounted its own live chart** (`OptionFocusPreview` → `WidgetView`). On a `timeseries` that is ~20
simultaneous recharts renders — visually noisy, slow, and a straight violation of the scope's
resolved decision #3 ("**one** configurable `OptionFocusPreview` with an `optionFocus` prop, not
per-option renderers"). The user called it out; direction (A) chosen: one preview, compact form.

## What changed

- **`options/OptionSectionCard.tsx`** — redesigned from "control + embedded chart" to a compact
  **row**: label + `Control` (block controls full-width underneath), the honest
  "no visible effect — renderer pending" note for DEAD options (per `optionLiveness`, untouched),
  and **no chart of its own**. New `onFocus(optionId)` callback fires on hover/focus-within so the
  host can point the one pinned preview at the option being edited; `focused` prop drives a subtle
  row highlight. Still zero card-local state — reads via `readOption`, writes via `writeOption`.
- **`wizard/OptionsStep.tsx`** — compact grouped form (group header + a bordered `divide-y` list of
  rows). Threads `onFocusOption`/`focusedOption`; no longer needs `ws`/`cell`/`refreshKey`.
- **`wizard/PanelWizard.tsx`** — owns `focusedOption` state. On the options step the pinned
  right-hand pane renders through **`OptionFocusPreview`** (same `WidgetView`, one render path) with
  `optionFocus` set to the focused option, plus a small chip naming it; other steps keep
  `PreviewPane` exactly as before (freeze behavior untouched).
- **`wizard/WizardTour.tsx`** — copy updated to the one-preview story (same two joyride stops).

Zero edits to `PanelEditor.tsx` / `FieldTab.tsx` / `OptionGroups.tsx` / `Control.tsx` /
`optionLiveness.ts` / `PreviewPane.tsx`.

## Rejected alternative

(B) one mini-preview per **group** (~3 charts). Rejected: still N renders, still redundant with the
pinned preview, and it is not what the scope committed to. `optionFocus` on the single preview gives
the same "see what this option does" teaching with one render.

## Tests (real gateway, rule 9)

- `wizard/optionsStep.gateway.test.tsx` — rewritten: harness mirrors PanelWizard (form + ONE
  `OptionFocusPreview`). Pins: exactly **one** `.option-focus-preview` mounts; a LIVE toggle
  (decimals) updates the single preview and sets `data-option-focus="decimals"`; a DEAD option
  still carries its note; the **cost model** (presentation toggle ⇒ no `viz.query` FETCH, counted
  through a delegating `ipc.invoke` spy) stays green.
- `options/optionSectionCard.gateway.test.tsx` — rewritten: row mounts NO preview/chart, LIVE/DEAD
  classification + note, `writeOption` write-through, `onFocus` reporting.
- `fieldTabBaseline.gateway.test.tsx` — **58/58 green, untouched**.
- `panelWizard`, `panelWizardSave`, `transformStep`, `optionFocusPreview` gateway suites — green.
- `npx tsc --noEmit` clean; lint has no findings in the touched files (repo-wide legacy errors
  pre-exist elsewhere).

## Addendum: barchart liveness rows (same day)

Picking **Bar chart** in the wizard threw `optionLiveness: no row for barchart/links` — the view was
never in `WIZARD_VIEWS`. Added per the declare + test pattern: `barchart` joins `WIZARD_VIEWS` with a
full table (only the 10 universal standard options register for it). LIVE: displayName/unit/decimals/
min/max/color/thresholds (the shared `valueFieldOptions`/`formatValue`/`categoryColor` bridge). DEAD +
render-proven in `fieldTabBaseline.gateway.test.tsx`: `mappings` (BarChartPanel never calls
`applyMappings`), `noValue` (the empty state is a hardcoded "no data yet"; proven on an UNSEEDED
series), `links` (the existing every-view links proof auto-extends via `it.each(WIZARD_VIEWS)`).
Baseline suite: 58 → **65 green**.

## Follow-ups (not this session)

- SourceStep UX: picking a SQL query / datasource kind is not discoverable (user report). Needs its
  own slice on the source-picker seam.
- Scope doc's "preview-per-option" phrasing predates this fix; STATUS.md now describes the
  one-preview surface.
