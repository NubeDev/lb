# HANDOVER — Build the panel wizard (preview-per-option, one engine with the Field tab)

**Status:** 🟢 READY TO BUILD. The baseline audit + the scope doc are shipped; the new session implements
Phase 1 of the wizard. No open questions — all long-term decisions are taken in the scope.

**Copy/paste this whole file into a fresh session.** Then read
[`docs/scope/frontend/dashboard/viz/panel-wizard-scope.md`](../../scope/frontend/dashboard/viz/panel-wizard-scope.md)
in full — it is THE spec; this handover is the map + the order.

---

## 0. The one-paragraph goal

A **stepped wizard for creating a panel** (source → chart type → small option sections), whose headline is
**preview-per-option** — each option card carries a live mini-preview of its effect on the user's data, so
live options show their effect instantly and dead options surface themselves. It is a **thin shell over the
existing panel model** (no second authoring surface, no drift), **isolated from data studio** (a new route;
zero edits to the current editor), and the engine it builds **ports back into the editor's Field tab** in
Phase 2. Backend concerns are already handled: presentation options re-shape cached frames (no gateway hit);
only data steps (source/chart-type/transform) re-query.

## 1. What's already done (do not redo)

- **The baseline audit.** `ui/src/features/dashboard/views/fieldTabBaseline.gateway.test.tsx` (24 tests,
  green) classifies every Field-tab option LIVE (observable render change) vs DEAD (stored + round-tripped,
  zero visible effect). Run: `cd ui && npx vitest run --config vitest.gateway.config.ts src/features/dashboard/views/fieldTabBaseline.gateway.test.tsx`.
  **Headline findings:** `timeseries` has 9 DEAD options (`mappings`, `links`, `custom.lineInterpolation`,
  `gradientMode`, `showPoints`, `spanNulls`, `axisPlacement`, `stacking.mode`, `thresholdsStyle.mode`);
  `table` has 4 DEAD per-column `custom.*`; the single-stat family is faithful. See
  [`debugging/frontend/field-tab-options-that-do-nothing.md`](../debugging/frontend/field-tab-options-that-do-nothing.md).
- **The scope doc.** [`scope/frontend/dashboard/viz/panel-wizard-scope.md`](../scope/frontend/dashboard/viz/panel-wizard-scope.md).
  Read it first; it has the goals, non-goals, the no-drift architecture, the example flow, the testing plan,
  the risks, and ALL resolved decisions.
- **Infra is up.** The vite dev server runs on 5173; the gateway on 8080. The built shell (4173) is NOT
  running — start it with `make ui-preview` only if you need playwright e2e.

## 2. The hard constraints (hold the line)

1. **No second authoring surface.** The wizard's working state IS `EditorState`
   (`ui/src/lib/panel-kit/cellEditorState.ts`). Every option writes through `writeOption`
   (`ui/src/features/panel-builder/options/binding.ts`). The preview resolves through `usePanelData`
   (`ui/src/features/dashboard/builder/usePanelData.ts`) + the `fieldconfig/format.ts` bridge. If a wizard
   step needs state the editor lacks, **extend `EditorState` for BOTH** — never add a wizard-only field.
2. **Isolated from data studio.** Phase 1 = a new route + new files only. **Zero edits** to
   `PanelEditor.tsx`, `FieldTab.tsx`, `OptionGroups.tsx`, `Control.tsx`, or any existing editor tab. The
   shared `OptionSectionCard` / `OptionFocusPreview` / `optionLiveness.ts` land ADDITIVELY under
   `ui/src/features/panel-builder/options/` (sibling to `OptionGroups.tsx`).
3. **Real samples, no fakes (rule 9).** Pre-source previews use REAL seeded rows (`seedIotDemo`-style —
   `cooler.temp`/`fryer.state` the gateway already seeds). After a source is picked, the user's real
   frames. Never a hand-written `*.fake.ts`.
4. **One responsibility per file (rule 8).** ≤400 lines, one verb per file. See the file plan in the scope's
   "How it fits the core → One responsibility per file."
5. **The baseline test stays green** throughout. The wizard neither implements nor removes any option in
   Phase 1, so the LIVE/DEAD classifications must not change.

## 3. The resolved decisions (already taken — do not re-litigate)

- **Dedicated route** entry point (`/t/$ws/dashboard/$d/new-panel`), NOT a Sheet. Deep-linkable, refresh-safe,
  maximally isolated. Uses the shipped @tanstack/react-router setup (see `scope/frontend/routing-scope.md`
  + an existing route file for the pattern).
- **One `OptionFocusPreview` component** with an `optionFocus: { optionId }` prop (NOT per-option renderers).
  It renders the SAME `WidgetView` (`ui/src/features/dashboard/views/WidgetView.tsx`) the dashboard renders,
  zoomed/highlighted to the region the option affects. One render path = no drift.
- **A declared `optionLiveness.ts` table** is the wizard's source of truth for the per-option `live: boolean`
  (drives the dead-option "no visible effect — renderer pending" note). The `fieldTabBaseline` test ENFORCES
  it: every registered option has a row (exhaustiveness), and each row matches rendered reality (accuracy).
  Declare + test — the project's house pattern.

## 4. Read these first (the seams you reuse)

| File | Why |
|---|---|
| `ui/src/lib/panel-kit/cellEditorState.ts` | The (de)serializer. Wizard state = `EditorState`. `cellToEditorState(defaultCell(view))` seeds a fresh wizard; `editorStateToCell(state, base)` finalizes on save. |
| `ui/src/features/panel-builder/options/registry.ts` + `binding.ts` | `optionsForView(view)` → the option list; `readOption`/`writeOption` → the read/write the wizard reuses verbatim. |
| `ui/src/features/panel-builder/options/OptionGroups.tsx` + `Control.tsx` | Today's Field-tab renderer (the thing Phase 2 replaces). `OptionSectionCard` composes the SAME `Control`. |
| `ui/src/features/dashboard/builder/usePanelData.ts` + `useVizQuery.ts` | The ONE data hook (invariant A). The wizard's full-panel preview + the `OptionFocusPreview` both ride it. |
| `ui/src/features/dashboard/views/WidgetView.tsx` | The render dispatch. `OptionFocusPreview` wraps it with `optionFocus`. |
| `ui/src/features/dashboard/views/fieldTabBaseline.gateway.test.tsx` | The contract. Copy its `plainDom`/`norm`/`setOpt` helpers for the wizard's no-drift + preview tests. |
| `scope/frontend/dashboard/viz/panel-wizard-scope.md` | THE spec. |
| `scope/frontend/dashboard/viz/field-config-scope.md` → "Known gaps" | The dead-option list (input to `optionLiveness.ts`). |

## 5. Build order (one commit per step, each green)

Each step lands green (tests + `tsc --noEmit` + `eslint src`) before the next. Do not batch.

1. **`optionLiveness.ts` + enforce it.** Declare the per-option `live` table from the baseline findings.
   Extend `fieldTabBaseline.gateway.test.tsx` with two assertions: (a) every registered option has a row;
   (b) each row matches reality (a LIVE row's option observably renders; a DEAD row renders byte-identical).
   This is the contract for every step after.
2. **`OptionFocusPreview.tsx`.** One component wrapping `WidgetView` with an `optionFocus` prop. Prove it on
   ONE option: focus `decimals` → the value readout renders large/isolated. Gateway test: render with
   `optionFocus: {optionId:"decimals"}` + decimals 2 → the focused preview shows "42.00".
3. **`OptionSectionCard.tsx`.** The reusable card: `<Control>` on the left, `<OptionFocusPreview>` on the
   right, the dead-option "renderer pending" note when `!live`. Prove ONE card (decimals) renders + its
   preview updates live. This is the engine both surfaces share.
4. **The wizard route + `SourceStep` + `ChartTypeStep`.** `/t/$ws/dashboard/$d/new-panel`; mounts
   `cellToEditorState(defaultCell("timeseries"))`. SourceStep reuses the Query tab's source picker; ChartTypeStep
   reuses `VizPicker.tsx`. Full-panel preview beside the steps.
5. **`OptionsStep` + `useWizardPreview`.** The stack of `OptionSectionCard`s for the chosen view. The
   preview hook debounces: presentation-option toggles call `viz.query` in inline-`frames` (shape-only) mode
   → NO gateway round-trip; data changes re-query. (This is the data-studio-ux fetch/shape split —
   `scope/frontend/dashboard/data-studio-ux-scope.md`.)
6. **`TransformStep`.** The data step (transformations re-query). Reuse the editor's `TransformTab`'s
   transform editors; offer the freeze-current-data toggle.
7. **react-joyride tour.** One dismissible concepts pass on first entry into OptionsStep. `pnpm add react-joyride`.
8. **Save.** `editorStateToCell(state, defaultCell)` → `dashboard.save`. Host re-checks the cap.

## 6. Mandatory tests (the testing plan, verbatim from the scope)

Real gateway, real seeded rows, no fakes (`*.gateway.test.tsx`):

- **No-drift invariant (headline).** Build a panel through the wizard's `writeOption` path AND through the
  editor's, for the same options; assert `editorStateToCell(wizardState) ≡ editorStateToCell(editorState)`.
- **Preview-per-option re-renders.** A LIVE option toggle (decimals) changes the mini-preview DOM; a DEAD
  option toggle (spanNulls) leaves it byte-identical. Reuse the baseline's `plainDom`.
- **Re-shape vs re-query (cost model).** Count gateway calls: a presentation toggle (decimals/threshold) →
  NO second `viz.query`; a data change (transform/chart-type) → DOES re-query.
- **Real samples, no fakes.** The pre-source mini-preview renders the REAL seeded `cooler.temp` value, not
  a hand-written literal.
- **Edit-cap gate + host backstop.** No `mcp:dashboard.save:call` → no wizard entry point; forced save →
  host denies (opaque).
- **Workspace isolation.** ws-B wizard's source picker + samples show only ws-B rows.
- **Baseline stays green.** `fieldTabBaseline.gateway.test.tsx` 24/24 unchanged.

Commands: `cd ui && npx tsc --noEmit` (clean), `pnpm lint` (test files are ignored by design — don't be
alarmed), `pnpm test:gateway` (real gateway), `pnpm test` (unit).

## 7. Definition of done — Phase 1

- A user can build any supported panel through the wizard without ever seeing the existing editor or typing
  JSON or a remembered field name.
- Every option the wizard exposes has a live mini-preview; dead options show the honest "renderer pending" note.
- Presentation-option toggles do not re-query the gateway (asserted).
- The no-drift invariant test is green.
- The existing editor + Field tab are UNTOUCHED (verify with `git diff` — no edits to `PanelEditor.tsx` /
  `FieldTab.tsx` / `OptionGroups.tsx`).
- The baseline test stays 24/24 green.
- Session doc + debug entries per CLAUDE.md; promote shipped truth to `public/frontend/dashboard.md`.

## 8. Phase 2 (NOT this session — separate handover)

Port the `OptionSectionCard` + `OptionFocusPreview` engine into the editor's Field tab, replacing today's
flat `OptionGroups` list. Then decide each dead option's fate (implement its render path, or remove it from
the per-view registry) against `optionLiveness.ts` + the preview's verdict. Edit deep-link (`?edit=<cellId>`)
lands on the same routing seam Phase 1 just established.

## 9. The two traps past me would fall into

- **"Just one wizard-specific field."** Every such temptation is the drift seed. Wizard state is `EditorState`;
  options are `OptionDef`s written via `writeOption`. If you need new state, extend `EditorState` for BOTH
  surfaces, not just the wizard.
- **Per-option preview renderers.** The first instinct is "a `<DecimalsPreview>`, a `<ThresholdsPreview>`, …".
  That's N renderers that drift from the real panel render. There is ONE `OptionFocusPreview` that wraps the
  real `WidgetView`. Hold this line.
