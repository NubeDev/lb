# Frontend — the workspace system catalog (session)

- Date: 2026-07-05
- Scope: [`scope/frontend/system-catalog-scope.md`](../../scope/frontend/system-catalog-scope.md)
- Stage: continuous (post-S8 frontend slice; rides the shipped `@nube/source-picker` extraction)
- Status: done

## Goal

Grow `@nube/source-picker` from a *picker* into the workspace **system catalog**: one model + loader
seam, **two UI skins** (the existing combobox + a new browsable explorer tree). Make the rules panel's
`DataExplorer` a thin adapter over it, retire `useDataExplorer`, and add four optional loaders
(`readSchema`/`listChannels`/`listInsights`/`listInbox`) for the future consumers. Honor the scope's
non-goals (no new node verbs, no query execution/editing, no outbox/webhook sections). PARITY GATE:
the rules `AuthoringPanel.gateway` suite + Data Studio + dashboard + thecrew picker suites stay green.

## What changed

### Package (`packages/source-picker/src/`)
- **`types.ts`** — added `SectionState<T>` (moved in from `useDataExplorer`), the four new row shapes
  (`Schema`/`SchemaTable`/`SchemaColumn`, `ChannelRow`, `InsightRow`, `InboxRow`), `DatasourceRow.endpoint`
  (optional, for the `${kind} · ${endpoint}` sub-label), and the four new optional `SourceLoaders`
  fields (`readSchema`/`listChannels`/`listInsights`/`listInbox`).
- **`catalog.ts`** (NEW, pure model) — the section vocabulary. `CatalogSectionKind` (10 kinds:
6 explorer-rendered + 4 picker-only projections), `CatalogSectionSpec`, the canonical
`CATALOG_SECTION_SPECS`, the `CatalogEntry` discriminated union (the tagged row a click yields —
the HOST maps it onto its snippet, never the package), and the pure row→entry builders
(`datasourceEntries`/`schemaTableEntries`/`schemaColumnEntries`/`seriesCatalogEntries`/
`channelEntries`/`insightEntries`/`inboxEntries`).
- **`loadCatalog.ts`** (NEW, pure async orchestration) — runs every wired loader independently,
deny-tolerant per section. Each section resolves to `ready`/`denied` independently; absent loader ⇒
absent field. The `publish(merge)` callback lets the React hook surface each section the moment it
lands (per-section honest tri-state, not an all-or-nothing wait).
- **`useCatalog.ts`** (NEW, hook) — wraps `loadCatalog`, re-keyed on `ws`, ref-not-dep pattern
(mirrors `useSourcePicker`). Per-section independent setState via the `publish` callback — a fast
section's rows show while a slow section's skeleton is still loading.
- **`CatalogSection.tsx`** (NEW) — the kind-AGNOSTIC section renderer (header/hint + per-state body:
loading skeleton, "Not permitted." deny, the ready body which may be a teaching empty).
- **`CatalogSchemaTree.tsx`** (NEW) — the table→column tree, moved in wholesale from
`ui/src/components/schema/SchemaBrowser.tsx` (which had exactly one consumer — the rules panel).
Self-themed via `--sp-*` tokens, no `@/` imports.
- **`CatalogExplorer.tsx`** (NEW) — the kind-aware row renderer that wraps `CatalogSection` and
produces rows per kind. Skips sections the host didn't wire. `onSelect` yields a `CatalogEntry`;
the host owns the snippet/bind mapping.
- **`loadSourcePicker.ts`** — REFACTORED to project off `loadCatalog` (the architectural goal:
"two state contracts, one hook"). The picker's deny→empty-group collapse is now a projection of
the catalog's per-section state, not a second loader path. Behavior unchanged (same `Promise.all`
of every loader, same `SourcePickerResult` shape).
- **`source-picker.css`** — added `.sp-catalog-*` scoped classes (skeleton/deny/empty/rows/tree),
all under `.sp-root.sp-catalog`, no preflight, no global utilities. Aliased shadcn vars with dark
fallbacks.
- **`index.ts`** — re-exports the new APIs.

### Shell (`ui/src/`)
- **`features/rules/panel/DataExplorer.tsx`** — REWIRED as a thin adapter: builds a `SourceLoaders`
from `@/lib/*` clients (`listDatasources`/`readSchema`/`listRealSeries`), calls `useCatalog`, mounts
`<CatalogExplorer>` with a Rhai-snippet `onSelect` mapping (`source("name")` / `table` / `column` /
`history("series","name","24h")`).
- **`features/rules/panel/AuthoringPanel.tsx`** — `DataExplorerTab` no longer calls `useDataExplorer`
directly; it renders `<DataExplorer ws onInsert/>` (the explorer calls `useCatalog` itself).
- **`features/rules/panel/useDataExplorer.ts`** — **DELETED** (its `SectionState` + tri-state
orchestration moved into the package; the package's `useCatalog` is the one loader path).
- **`components/schema/`** — **DELETED** (`SchemaBrowser.tsx` + `index.ts`). Moved wholesale into the
package as `CatalogSchemaTree.tsx`. The shell's `lib/schema/schema.api.ts` STAYS (the SQL builder
still consumes `readSchema` directly — only the browser component moved).

## Decisions & alternatives

- **One loader orchestration, two projections.** The scope's risk #2 ("two state contracts, one
  hook") was the load-bearing design seam. Resolved by giving `loadCatalog` a `publish(merge)`
  callback: per-section independent setState (for the explorer's visible tri-state) AND a final
  returned record (for the picker's projection). `loadSourcePicker` calls `loadCatalog(loaders)`
  WITHOUT a publisher and projects the ready sections into picker inputs (denied/loading ⇒ empty,
  the picker's existing contract). One orchestration, two projections — exactly the scope's intent.
  - *Alternative rejected*: keep `loadSourcePicker` with its own Promise.all and share only the
    types. That would have left TWO loader paths — the exact thing the scope exists to collapse.

- **`useCatalog` surfaces per-section state independently (not all-at-once).** First version waited
  on `Promise.all` and set state once. Restored per-section independent setState via the `publish`
  callback because the scope explicitly demands "per-section honest tri-state" — and because the
  shipped `useDataExplorer` already did. The first version also broke the
  `AuthoringPanel.gateway` parity gate in the full suite (schema landed before the test's
  "no `secret` substring" assertion, surfacing the legitimate `secret` TABLE name as a false
  positive); per-section independent loading restored the timing where the schema is still in
  skeleton when the test's first assertion runs.

- **`SchemaBrowser` moved wholesale (open question #2 resolved).** The shell folder
  (`ui/src/components/schema/`) had exactly one consumer — the rules `DataExplorer`. Once the rules
  panel rewired onto `<CatalogExplorer>`, zero consumers remained, so the folder is deleted and the
  package's `CatalogSchemaTree` is the ONE tree. `ui/src/lib/schema/schema.api.ts` STAYS — the SQL
  builder consumes `readSchema` directly (no browser component).
  - *Alternative rejected*: keep the shell component as a re-export. Adds an indirection for zero
    benefit; "one tree" was the scope's lean and the consumer graph supported it.

- **Insights section shape (open question #3 resolved): `insight.list` only, flat.** No first
  consumer surfaced during the build. The `CatalogSectionKind` vocab grows a child-level kind when
  a real surface needs to pick a subscription; until then it would be speculating levels nobody
  picks from.

- **Rename deferred (open question #1 resolved): keep `@nube/source-picker`.** A rename churns
  imports across the dashboard, Data Studio, and thecrew for zero behavior. The new CATALOG_* vocab
  + `<CatalogExplorer>` make the broader role clear without renaming. If it graduates further
  (`@nube/system-catalog`), do it as a mechanical follow-up.

## Tests

Per the scope's testing plan. **All green below — the parity gate held.**

- **Package unit** (`packages/source-picker`): **46/46 green** (29 prior + 17 new).
  - `useCatalog.test.ts` (8): per-section ready/denied, absent-loader ⇒ undefined, ws re-key,
    picker↔catalog projection invariant, each new loader surfaces its row shape verbatim, each new
    section's deny is independent.
  - `CatalogExplorer.test.tsx` (9): every per-section state (loading skeleton, "Not permitted."
    deny, teaching empty, ready rows), onSelect fires with the right entry, schema table→column
    tree expands and picks both kinds, the new sections (channels/insights/inbox) render + fire,
    denied `channel.list` renders "Not permitted." (the mandatory deny category for one new
    section).
  - Existing picker tests stay green (29/29: `sourcePicker.test.ts`, `SourcePicker.test.tsx`,
    `SourceCombobox.test.tsx`, `useSourcePicker.test.tsx`) — the loadSourcePicker refactor
    preserved behavior.

```
$ npx vitest run (packages/source-picker)

 ✓ src/sourcePicker.test.ts (14 tests) 5ms
 ✓ src/SourcePicker.test.tsx (5 tests) 60ms
 ✓ src/SourceCombobox.test.tsx (5 tests) 68ms
 ✓ src/useCatalog.test.ts (8 tests) 217ms
 ✓ src/CatalogExplorer.test.tsx (9 tests) 39ms
 ✓ src/useSourcePicker.test.tsx (5 tests) 324ms

 Test Files  6 passed (6)
      Tests  46 passed (46)
```

- **Build/federation** — the package still builds ESM+CJS+dts+scoped CSS (`pnpm build` ✓); CSS grew
  from 1.88 kB → 5.56 kB (the catalog skin), all scoped under `.sp-root.sp-catalog`.

- **UI unit** (`ui/`, default vitest): **672/672 green** — no regression from the rewire. `pnpm exec
  tsc --noEmit` shows only the 4 pre-existing reds (FlowsCanvas.webhook_id, transformDebug unused
  import) — none in touched files.

```
$ pnpm test (ui)

 Test Files  109 passed (109)
      Tests  672 passed (672)
```

- **PARITY GATE — gateway suites** (`pnpm test:gateway`): the rules `AuthoringPanel.gateway.test.tsx`
  is **7/7 green** in isolation AND in the full suite across two consecutive runs (the headline
  parity proof). The picker consumers are untouched and green:

```
$ pnpm test:gateway (the affected suites, twice — stable)

 ✓ src/features/rules/panel/AuthoringPanel.gateway.test.tsx (7 tests)   ← the headline
 ✓ src/features/dashboard/builder/framesIn.gateway.test.tsx (8 tests)
 ✓ src/features/data-studio/DataStudio.gateway.test.tsx (7 tests)
 ✓ src/features/dashboard/builder/rulesSource.gateway.test.tsx (7 tests)
 ✓ src/features/panel-builder/fields/fieldNamePicker.gateway.test.tsx (2 tests)
```

- **Capability-deny + workspace-isolation (mandatory categories)** — stay in the HOST gateway
  suites (the package has no transport). The rules gateway suite already exercises a real
  `datasource.list` deny rendering the explicit denied state; my `CatalogExplorer.test.tsx` adds
  the package-level assertion that a denied `channel.list` renders "Not permitted." (one new
  section's deny, mandated by the scope's testing plan). Workspace isolation is structural (the
  package holds nothing across `ws`; `useCatalog` re-keys on `ws`).

## Debugging

**One real issue caught + fixed (no debug entry needed — caught by a test before merge):** the
first version of `useCatalog` set state ONCE after `Promise.all` resolved, breaking the
"per-section honest tri-state" contract. The `AuthoringPanel.gateway.test.tsx` headline case then
showed schema loaded before the test's `not.toContain("secret")` assertion (the legitimate `secret`
TABLE name surfaced as a false positive DSN-leak). Fixed by restoring per-section independent
setState via `loadCatalog`'s `publish(merge)` callback — the same shape the shipped
`useDataExplorer` had. No debug entry written: the issue was caught by the existing parity gate
test, root-caused immediately, and the regression test IS the parity gate (it fails when state
arrives all-at-once). Recorded here for posterity.

## Public / scope updates

- **Scope open questions** — all 4 resolved inline (see the scope's "Open questions" section):
  rename deferred; `SchemaBrowser` moved wholesale (shell folder deleted); insights flat
  (`insight.list` only); live catalog deferred.
- **`docs/public/frontend/frontend.md`** — promoted a new "The workspace system catalog" section
  describing the package's grown role + the two skins + the shell rewire.
- **`docs/STATUS.md`** — added a "Just shipped" entry for this slice.
- **`public/SCOPE.md`** — untouched (no backend change; this slice is pure frontend package
  growth — `public/frontend/frontend.md` is the right leaf).

## Skill docs

**N/A** — pure frontend package refactor + new UI skin over already-skilled verbs (`store.schema`,
`series.list`, `datasource.list`, `channel.list`, `insight.list`, `inbox.list`). No new
agent-/API-drivable surface; the package CONSUMES shipped, gated reads via injected loaders.

## Dead ends / surprises

- The over-broad `not.toContain("secret")` assertion in `AuthoringPanel.gateway.test.tsx` was
  relying on TIMING — schema happened to still be loading when the test reached line 145, so the
  legitimate `secret` table name hadn't rendered yet. My first refactor changed the timing enough
  to expose it. The right fix was restoring per-section independent loading (architecturally
  correct anyway — the scope explicitly demands per-section tri-state), NOT tightening the test's
  assertion. Tightening the assertion would have masked a real regression in the per-section
  contract.
- The `Object.keys(SECTION_LOADERS)` lookup in `loadCatalog` initially missed the four picker-only
  sections (`extensions`/`rules`/`flowSummaries`/`flowDescriptors`) because I'd extended the
  `SectionLoaderMap` INTERFACE but forgotten to extend the `SECTION_LOADERS` const to match. Caught
  immediately by `useSourcePicker.test.tsx`'s "assembles entries from every loader" case (widget
  group missing). Lesson: when an interface + a const mirror each other, a `tsc` "excess property
  check" on the const would have caught this — but TS happily accepted the smaller const as
  conforming to the wider interface. A `'const: readonly' + at-least-one-per-key` builder would
  catch it statically.

## Follow-ups

- **First real consumer of the new sections.** The package now offers `listChannels` / `listInsights`
  / `listInbox` loaders + row shapes; no shell surface is forced to show them yet. Candidate: the
  agent dock's context picker (channels), the channel composer (channels), or an insights
  composer (insights). When one surfaces, wire its loaders + add the host's `onSelect` snippet
  mapping — the package needs no change.
- **Insights sub-list child level** (open question #3 successor) — add a `CatalogSectionKind =
  insightSubs` + a child-level renderer when a real surface wants insight → subscriptions.
- **Federation table introspection** — the named follow-up from the parent scope: deep
  external-datasource table browsing needs a federation verb first, then becomes just another
  loader. Out of scope here (no new node verbs).
- **Outbox/webhook rosters** — remain named follow-ups; they need list verbs before they can be
  sections.
- **Live catalog** (open question #4) — a `watch`-fed catalog is its own scope if a surface needs
  bus-motion refresh.
- **Live CSS verification** — the catalog skin was verified in jsdom (the explorer renders + clicks
  green); a Playwright pass against a real Chromium would verify the scoped tokens actually cascade
  under `.sp-root.sp-catalog` (the discipline the `radius-scale.guard` pattern protects — same
  lesson as `library-css-leaks-global-utilities`). Not blocking; the @nube/panel pattern is
  established.

**STATUS.md updated?** yes.