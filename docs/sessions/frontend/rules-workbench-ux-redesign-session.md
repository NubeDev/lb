# Session: rules workbench UX redesign (Save-always + result feedback)

**Area:** frontend · **Date:** 2026-07-03 · **Scope:** rules-editor-ux

## The ask

User ran `query("timescale", "select * from site")` in the rules workbench, saw the 3-row grid
but no confirmation it worked, and reported the whole UX as bad — "I can't even save a rule."
Asked for a full workbench redesign using the impeccable design skill.

## What was wrong (from the code, not guesses)

- **Save unreachable on an ad-hoc buffer** — Save/Rename gated on `selectedId`; a fresh buffer
  had no Save control. See [debugging entry](../../debugging/frontend/rules-save-unreachable-on-adhoc-buffer.md).
- **No run-completion feedback** — bare scroll box, no status, success didn't read as success.
- **Off-system error state** — raw `red-50/red-300` instead of the shared `destructive` token.
- **Grid was plain** — no sticky header, no zebra, blanks for NULL.

## Changes

| File | Change |
|------|--------|
| `useRules.ts` | `saveCurrent(nameForNew?)` (update-in-place / needs-name); `hasRun` flag reset on open/new/example, set after a run. |
| `RulesView.tsx` | Save **always** in toolbar + global `⌘S`; dirty-aware variant; one inline name field for rename *and* name-on-first-save; ResultBar over the result region. |
| `ResultBar.tsx` (new) | Run-state dot + summary (`3 rows · 4 ms` / Running… / Failed / Not run yet). |
| `RunResult.tsx` | Skeleton (not spinner); destructive-token error with a "Rule failed" head; idle vs finished-empty copy via `hasRun`. |
| `GridTable.tsx` | Sticky header on panel tone, zebra rows, tabular mono cells, `NULL` as a dim literal, footer keeps `showing N of M`. **`cellAt` reads BOTH row shapes** (object-keyed platform rows AND array federation rows) — the NULL-cells fix. |
| `JsonView.tsx` (new) | Verbatim pretty-printed `RunResult` JSON + a copy button — the raw-shape view. |
| `rules.types.ts` | `grid.rows` widened to `(Record<string,unknown> \| unknown[])[]` — federation rows are arrays. |

## Second bug found + fixed (federated grid = all NULL)

Rendering absent cells as a visible `NULL` (part of the redesign) surfaced a **pre-existing** data
bug: `query("timescale", …)` showed "3 rows" + correct headers but every cell was NULL. Cause:
platform rows are objects keyed by column name; **federation rows are column-aligned arrays** (the
sidecar re-projects Arrow objects to arrays — `rust/extensions/federation/src/query.rs`).
`GridTable` did `row[columnName]`, which is `undefined` on an array row. Fixed with `cellAt` reading
by index for arrays / by key for objects. Full write-up in the
[debugging entry](../../debugging/frontend/rules-save-unreachable-on-adhoc-buffer.md#follow-up-same-session-federated-grid-rendered-every-cell-null).

## JSON toggle (user ask)

Added a `table | json` segmented toggle in the ResultBar (shown only with a result). `json` renders
`JsonView` — the verbatim `RunResult` pretty-printed, with a clipboard copy — so an author can see
the exact returned shape (incl. the row shape that differs platform vs. federation) and copy it.
View state is owned by `RulesView` so the toggle and body stay in sync.

## Design decisions

- **Kept the warm cream `--bg` (`40 30% 96%`) and ochre accent.** These are committed brand
  tokens, not AI-default cream — identity-preservation wins (impeccable: skip the palette step
  when committed brand colors exist). PRODUCT.md register is `product` → Restrained: accent
  carries primary action + selection + state only.
- **No modal.** Name-on-first-save is an inline field, per the product register's "modal as
  last resort."
- **Preserved test contracts.** Kept the `grid count` "showing N of M" text and the
  `confirm rename rule` aria-label so the real-gateway test's existing assertions still bind.

## Verification

- `pnpm exec tsc --noEmit` — no new errors in the rules files (only the repo-wide pre-existing
  lucide JSX-types noise remains).
- `pnpm exec vite build` — **clean**, all 4821 modules transformed, bundle emitted.
- Token-faithful static render screenshotted (scratchpad `preview.png`): Save visible in the
  toolbar, `● Result · 3 rows · 4 ms` status bar, sticky/zebra grid.
- **UI gateway suite now runs** — the repo-wide `Invalid Chai property: toBeInTheDocument` break
  was fixed this session (jest-dom's `/vitest` entry extended a different `expect` instance than the
  runner's; both setup files now `expect.extend` the matchers onto the runner's own `expect`). See
  the [debugging follow-up 2](../../debugging/frontend/rules-save-unreachable-on-adhoc-buffer.md#follow-up-2-same-session-the-ui-test-suite-was-red-repo-wide--jest-dom-matchers-not-attaching).
  Default suite **386/386 green**; **rules gateway 7/7 green** (validates Save-always, the federated
  grid render, and the JSON toggle against a real spawned node). 8 gateway tests remain red across
  proof-panel/system/dashboard — pre-existing, unrelated (ext-sidecar prereq + suite-ordering), not
  jest-dom and not rules.

## Follow-ups

- ~~Fix the jest-dom / vitest matcher break~~ — **done this session** (see above); rules gateway
  7/7 green.
- Add gateway assertions for the new behavior: Save visible with no rule open; `⌘S` on a fresh
  buffer opens the "Save as" field; ResultBar summary + `table|json` toggle after a grid run;
  a federated (array-row) grid renders real cell values (regression for the NULL bug).
- Wrap the `⌘S` keydown handler's state updates to silence the React `act(...)` warning in the
  rules gateway test (non-fatal; tests pass).
- The 8 remaining gateway failures (proof-panel / system / dashboard) want their own look:
  proof-panel needs its extension sidecar built; the others pass in isolation but interfere under
  the shared-real-gateway serial run (test-isolation).
