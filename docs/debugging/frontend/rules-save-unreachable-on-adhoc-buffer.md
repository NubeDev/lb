# Rules workbench: Save is unreachable on an ad-hoc buffer

**Area:** frontend · **Status:** resolved · **Date:** 2026-07-03

## Symptom

A user opened the rules workbench, ran `query("timescale", "select * from site")` (which
succeeded — 3 rows in the grid), and reported "I can't even save a rule." The toolbar showed
only **Run**. No Save button was present, and there was no clear confirmation the run had
succeeded.

## Root cause

Two separate defects, both in the workbench UX (not the engine — the query ran and returned
the 3 seeded sites correctly):

1. **Save was conditional on `selectedId`.** In `RulesView.tsx` the Save and Rename buttons
   rendered only inside `if (r.selectedId)`. A freshly authored, not-yet-saved buffer has
   `selectedId === null`, so Save was *absent from the DOM* exactly when the user most needed
   it — after typing a rule they wanted to persist. Creating a rule required discovering the
   rail's separate "New rule" name-first form; there was no "save what I'm looking at" path
   and no `⌘S`.

2. **No run-completion feedback.** The result region was a bare `max-h-[45%]` scroll box with
   no header. A successful grid run produced a table but nothing that read as "it ran, here's
   what it returned," so a successful run didn't register as success. There was also no way to
   tell "you haven't run yet" from "ran and returned nothing."

## Fix

- `useRules`: added `saveCurrent(nameForNew?)` — the single "persist what I'm looking at"
  action. An open rule updates in place; a fresh buffer returns `{ needsName: true }` so the
  caller can reveal an inline name field rather than silently failing. Added a `hasRun` flag so
  the result region distinguishes first-load idle from a finished empty run.
- `RulesView`: Save is now **always** in the toolbar, wired to `saveCurrent` and to a global
  `⌘S` / `Ctrl+S` handler; it's dirty-aware (prominent when there are unsaved edits or the
  buffer is new, quiet outline + "Saved" when clean and open). One inline name field serves
  both rename and name-on-first-save.
- New `ResultBar` status header over the result region: a run-state dot + a summary
  ("3 rows · 4 ms" / "Running…" / "Failed" / "Not run yet").
- `RunResult`/`GridTable`: token-correct states (destructive token instead of raw `red-*`,
  skeleton instead of spinner-in-content, sticky/zebra table, `NULL` rendered as a dim literal
  instead of a blank).

## Regression

The real-gateway test (`RulesView.gateway.test.tsx`) already exercises create/rename/run over a
real node; the Save-always contract is asserted there via the persistent `save rule` control and
the `grid count` "showing N of M" footer (preserved verbatim so the existing assertion holds).

> NOTE: the UI gateway suite currently fails repo-wide with
> `Invalid Chai property: toBeInTheDocument` — the `@testing-library/jest-dom` matchers aren't
> attaching under the current vitest/expect versions. This is pre-existing (it hits untouched
> files across `channel/`, `dashboard/`, theme tests too) and is unrelated to this change;
> validation here was the production `vite build` (clean) plus a token-faithful visual render.
> The jest-dom harness break is worth its own entry.

## Lesson

A "save what I'm working on" action must not be gated on the artifact already being saved — that
inverts the flow (you can only save what's already saved). Make the primary persist action
always reachable (toolbar + `⌘S`), and let an un-named draft prompt for a name inline rather than
hiding the control.

---

## Follow-up (same session): federated grid rendered every cell NULL

After the redesign made empty cells render as a dim `NULL` literal (instead of a blank string),
a **pre-existing** data bug became visible: running `query("timescale", "select * from site")`
showed the correct headers (`id`, `name`) and "3 rows" in the status bar, but **every cell read
`NULL`**. (The original UI had the same bug — it just rendered blanks, so nobody noticed the data
was missing.)

**Root cause — two row shapes, one renderer.** A grid result's `rows` arrive in *different shapes*
depending on the source engine:

- **platform** (SurrealDB) rows are **objects keyed by column name** — `{ id: "site-001", … }`
  (`store_query/run.rs`, `columns_of` unions the object keys).
- **federation** (datasource) rows are **column-aligned arrays** — `["site-001", …]`. The sidecar
  deliberately re-projects Arrow objects to positional arrays
  (`rust/extensions/federation/src/query.rs`: *"re-project to a column-aligned array so the wire
  shape is `{columns:[...], rows:[[...], ...]}`"*).

`GridTable` (and the wire type `RuleOutput.grid.rows`) assumed only the object shape and did
`row[columnName]`. On an **array** row that's `row["id"]` → `undefined` → NULL for every cell.
No test caught it: the Postgres federation test only asserts `rows.len()`, never a cell value/key.

**Fix (client renderer).** `GridTable.cellAt(row, column, index)` reads by **index when the row is
an array, by key when it's an object**. The wire type was widened to
`rows: (Record<string, unknown> | unknown[])[]` to make both shapes honest.

**Lesson.** When two producers feed one renderer, pin the *shape* in a test, not just the count —
`rows.len() == 5` passes whether the cells are populated or all-NULL. And rendering absent data as
a visible `NULL` (not a blank) is what surfaced a bug that had been silently shipping.

---

## Follow-up 2 (same session): the UI test suite was red repo-wide — jest-dom matchers not attaching

The prior two fixes couldn't be validated in-test because the WHOLE UI suite (default + gateway)
failed with `Invalid Chai property: toBeInTheDocument` / `toBeDisabled` — every `@testing-library/
jest-dom` matcher threw. 17 default + 119 gateway tests down, on files nobody had touched.

**Diagnosis (by probe tests).**
- A star import `import * as m from "@testing-library/jest-dom/matchers"` under Vitest returns all
  33 matchers correctly — so Vite resolves the module's internal re-export chain fine.
- Manually `expect.extend(m)` in a test file makes `toBeInTheDocument` a real function and the
  assertion passes.
- But the shipped setup did `import "@testing-library/jest-dom/vitest"`, whose entry is
  `import { expect } from "vitest"; expect.extend(extensions)`. That `expect` resolved to a
  DIFFERENT instance than the one the runner hands each test file, so its `extend` silently no-oped.
  (Dual-instance: the `/vitest` convenience entry's `expect` ≠ the runner's `expect` here.)

**Fix.** In BOTH setup files (`src/test/setup.ts`, `src/test/setup-gateway.ts`) replace
`import "@testing-library/jest-dom/vitest"` with:

```ts
import * as jestDomMatchers from "@testing-library/jest-dom/matchers";
import { expect } from "vitest";
expect.extend(jestDomMatchers);
```

— extend the `expect` WE import (the runner's real instance), binding the matchers where the tests
actually assert.

**Result.** Default suite **17 failed → 0 (386/386 pass)**. Gateway suite **119 failed → 8**; the
remaining 8 are pre-existing and unrelated (proof-panel needs its extension sidecar built;
SystemView/DashboardView/sqlSource pass in isolation but interfere under the shared-real-gateway
serial run — a test-isolation issue, not a matcher error). The **rules gateway suite passes 7/7**,
validating the Save-always + federated-grid + JSON-toggle work against a real node.

**Lesson.** A library's `/vitest` (or `/jest`) auto-setup entry that internally imports the runner's
`expect` can bind a different instance than your tests use — the matchers "load" but no-op. When
matchers mysteriously don't attach, extend the runner's own `expect` explicitly with the matchers
map rather than relying on the convenience side-effect import.
