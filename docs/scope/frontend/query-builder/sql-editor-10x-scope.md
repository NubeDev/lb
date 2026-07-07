# SQL editor 10x scope — schema-aware completion, multi-statement splitting, and formatting on CodeMirror

Status: scope (the ask). Slice 2 of [`query-builder-10x-scope.md`](query-builder-10x-scope.md). Promotes
to `public/frontend/query-builder.md` on ship.

The Code half of the builder is `SqlEditor.tsx` — a `@uiw/react-codemirror` editor with the stock
`@codemirror/lang-sql` grammar and **no schema awareness, no formatting, no statement splitting**.
Tabularis's editor (Monaco) has schema-aware IntelliSense, a Format action, and a dialect-aware
multi-statement splitter. This slice ports those **three behaviours onto our existing CodeMirror** — no
Monaco. Two of the three are library-agnostic pure TS (splitter, formatter) and drop in as-is; the third
(completion) uses a capability `@codemirror/lang-sql@6.9.1` **already has**.

## The decision up front: CodeMirror, not Monaco

We stay on CodeMirror. Rationale (this is umbrella OQ #1 — the one decision most worth a human
confirmation, recorded here decisively):

- **`@codemirror/lang-sql` already accepts a schema for completion.** `sql({ dialect, schema, tables,
  defaultTable })` gives table + column completion out of the box — the exact feature we're porting. We
  feed it the `Schema` we already load. No new provider machinery, ~0 new dependency.
- **It's the app's editor.** `useCodeMirrorTheme` themes it; it's used across the app; the theme integrates
  with the shipped theme-customizer tokens. Monaco's theme is a **global singleton** — one theme for every
  editor instance on the page — which fights the multi-pane Dockview Data Studio (slice 3's home).
- **Bundle + runtime.** Monaco is ~2 MB + a web worker; CodeMirror is already loaded. For a SELECT-only
  editor, Monaco's extra weight buys signature-help and stronger context inference we don't need in v1.

**Rejected: swap to Monaco.** Documented as viable if, after this slice ships, the team wants a full
IDE-grade editor (ghost-text AI completion, hover types, diagnostics squiggles). That's a clean future swap
behind the same `SqlEditor` prop surface — not a v1 need. Flagged as the open question, not blocking.

## Goals

- **Schema-aware completion.** Typing a bare token in a statement offers the workspace's **tables**; typing
  `table.` (or `alias.`) offers **that table's columns** with their types; SQL **keywords** are offered.
  Fed by the same `Schema` the builder already loads (`store.schema` local, `federation.schema` projection
  for federation) — one schema source, both dialects.
- **Format.** A "Format" button pretty-prints the current SQL via `sql-formatter`, dialect-mapped
  (`postgresql`/`sqlite` for standard, a sensible default for surreal), `keywordCase: 'upper'`.
- **Multi-statement splitting.** A dialect-aware splitter (ported `sqlSplitter`) partitions the editor text
  into statements so the UI can **run/preview the statement under the cursor** (or the selection) instead of
  the whole buffer — matching Tabularis's "Execute Selection" ergonomics. Splitting is *selection*, never
  batch *execution* — the host still runs exactly one SELECT.

## Non-goals

- **No Monaco** (see the decision above).
- **No error squiggles / diagnostics.** Tabularis itself sets no markers; errors surface in the results
  pane. We keep that — a denied/failed Run shows the host error in the results area (slice 3), not inline.
- **No AI inline completion / text-to-SQL in the editor.** Tabularis has none wired into its editor either.
  Our `sql.generate`-via-MCP button is an already-named, separate follow-up (the `SqlEditor` header dropped
  it on port) — out of scope.
- **No multi-statement *execution*.** The splitter picks one statement to run; the host stays SELECT-only,
  single-statement, parse-allowlisted. No batch run, no transaction UI.
- **No new backend / verb / cap.** Completion reads the already-loaded `Schema`; formatting + splitting are
  pure client TS.

## Intent / approach

### 1. Schema-aware completion — `SqlEditor.tsx` + `sqlCompletion.ts`

`@codemirror/lang-sql`'s `sql()` config takes a `SQLConfig` with `schema`/`tables`. Add a small pure module
that projects our `Schema` (`{tables:[{name,columns:[{name,type}]}]}`) into that shape, and pass it into the
existing `extensions={[sql(...), ...cm.extensions]}` call:

```ts
// ui/src/features/dashboard/builder/editors/sqlCompletion.ts  (pure, no React)
import type { SQLConfig } from "@codemirror/lang-sql";
import type { Schema } from "@/lib/schema";
/** Project our Schema into @codemirror/lang-sql's completion config. */
export function schemaConfig(schema: Schema): Pick<SQLConfig, "schema" | "tables"> { … }
```

`SqlEditor` gains an optional `schema?: Schema` prop (absent ⇒ today's behaviour, no completion — honest
degrade). `RawEditor.tsx` passes the `schema` it already receives via `SqlQueryEditor` straight through, so
Code mode gets the **same schema the Builder dropdowns use** — one load, both halves. Dot-trigger and
keyword completion come free from the lang package once `schema` is set; the projection is the only new
code. The completion is **workspace-walled by construction** — it can only offer what the walled `Schema`
contains.

> Verify during build: pin the exact `SQLConfig` shape against the installed `@codemirror/lang-sql@6.9.1`
> (`schema` is a `{[table: string]: readonly (string | Completion)[]}` or the newer `SQLNamespace`; the
> projection adapts to whichever the pinned version exports). This is a 1-file adapter — keep it pinned by
> a unit test so a lang-sql bump can't silently break completion.

### 2. Format — `sqlFormat.ts` + a header button

Port Tabularis's `sqlFormat.ts` (a thin `sql-formatter` wrapper; near-zero coupling). `sql-formatter@^15`
is the one new `ui/package.json` dependency. Map dialect → the formatter's language
(`standard`→`postgresql`/`sqlite`). Wire a **Format** button in `SqlQueryHeader.tsx` (Code mode only) that
formats `value.rawSql` in place. Formatting a hand-edited string never changes meaning, so no confirm.
**Peer-review fix: the Format button is shown only for `dialect === "standard"` in v1** — `sql-formatter`
has no SurrealQL grammar and its `sql` fallback can mangle Surreal syntax (`table:id` record ids, `type::`
functions, `->` graph traversal), which *would* change meaning. A SurrealQL formatter is a deferred item;
until then, honest absence beats a corrupting button.

### 3. Multi-statement splitting — port `sqlSplitter/` verbatim

`/tmp/tabularis/src/utils/sqlSplitter/` is **pure TS, zero coupling** (no Tauri, React, or Monaco) — copy it
into `ui/src/lib/sql/split/` (`index.ts`, `tokenizer.ts`, `splitter.ts`, `classify.ts`), keeping its
`Dialect` union and `splitStatements`/`splitQueries`/`isSelect`/`returnsResultSet` API. It handles `$$`
blocks, `DELIMITER`, `GO`, dollar-quoting, comment folding — far beyond a naive `;`-split. Map our
`SqlDialect`→ its `Dialect` (`standard`→`postgres`/`sqlite`, `surreal`→`generic`). Consumer: slice 3's Run
uses `splitStatements(text, dialect)` to find the statement at the cursor and runs that one.

> This is the single most portable, highest-leverage steal in the whole harvest — pure, tested-in-Tabularis,
> immediately reusable, and useful beyond the builder (any future SQL surface). Copy it with attribution in
> the file header (Apache-2.0; see §Licensing below).

## How it fits the core

- **Rule 6 (tenancy).** Completion offers only the walled `Schema`; no cross-tenant leakage possible — it's
  a client projection of a workspace-pinned read.
- **Rule 5 (caps).** No new cap. If `store.schema`/`federation.schema` is denied, `schema` is empty and
  completion silently offers nothing (honest degrade) — the Code editor still works. Pinned in slice 3's
  capability-deny gateway test.
- **Rule 9 (no mocks).** The projection, formatter, and splitter are pure — unit-tested with fixtures, no
  fakes. The real schema-fed completion is exercised in slice 3's real-gateway mount.
- **Rule 8 (one file).** `sqlCompletion.ts` (projection), `sqlFormat.ts` (formatter), `lib/sql/split/*`
  (splitter, already one-verb-per-file) — each its own file. `SqlEditor.tsx`/`RawEditor.tsx`/`SqlQueryHeader.tsx`
  gain a prop/button each, staying well under 400 lines.
- **Rule 10.** No extension branching — dialect is config, schema is data.
- **Symmetric / one datastore / state-vs-motion / durability / secrets / SDK.** N/A (pure UI + TS).
- **Skill doc.** N/A — no new drivable verb.

## Example flow

1. `SqlQueryEditor` passes its `schema` prop into `RawEditor` → `SqlEditor`. In Code mode the user types
   `SELECT `; a keyword/table completion list appears. They type `point_reading.` → `site_id, ts, value, …`
   with types.
2. They click **Format**; `sql-formatter` reflows the statement (`keywordCase: upper`, indented).
3. They paste two statements. The splitter marks the statement under the cursor; **Run** (slice 3) runs only
   that one through `store.query`/`federation.query` (single SELECT, host-leashed).

## Testing plan

Per `scope/testing/testing-scope.md`. Pure-TS slice; mandatory capability-deny/workspace-isolation are
covered by slice 3's real-gateway test (empty schema ⇒ empty completion). Here:

### Unit (pure)

- **`sqlCompletion.schemaConfig`** — given a `Schema`, the config exposes the right tables and each table's
  columns; an empty `Schema` yields empty completion (the degrade contract). Pin the `SQLConfig` shape
  against the installed lang-sql version.
- **`sqlFormat`** — a known messy SQL → the expected formatted string per dialect mapping; formatting is
  idempotent (format∘format = format).
- **Splitter** — port Tabularis's own test cases (or a representative subset): `;`-split, `$$` block,
  `DELIMITER`, comment-only fragment attaches to the next statement, `isSelect`/`returnsResultSet`
  classification; `SqlDialect`→`Dialect` mapping.

### Integration (in slice 3's gateway test, not duplicated here)

- Real-gateway mount of the Code editor with a real `store.schema`/`federation.schema` load: completion
  offers the seeded demo tables/columns; an unauthorized session offers nothing (deny) — proving the
  end-to-end schema feed, not a fake.

## Risks & hard problems

- **`@codemirror/lang-sql` schema API shape** — versions differ (`schema` map vs `SQLNamespace`). Pin the
  projection with a unit test against the installed version; adapt the adapter, not the caller.
- **CodeMirror completion ≠ Monaco IntelliSense** — no signature help, weaker mid-expression inference.
  Accept for v1; the Monaco open question records the escape hatch.
- **Formatter dialect fidelity** — `sql-formatter` has no SurrealQL dialect; the `sql` default is close
  enough for a SELECT (cosmetic). A SurrealQL formatter is a deferred polish item.
- **Splitter dialect mapping for Surreal** — `generic` is safe (SurrealQL is `;`-separated for our subset).
- **Bundle** — `sql-formatter` (~small) is the only add; the splitter and completion add no dependency.

## Licensing

Tabularis is **Apache-2.0**. The `sqlSplitter` (and `sqlFormat` wrapper) ports must carry an attribution
line in each file header ("Ported from Tabularis (github.com/TabularisDB/tabularis), Apache-2.0") per the
license's notice requirement. Confirm the repo's third-party attribution convention during build (check for
an existing `NOTICE`/`THIRD-PARTY` file; the shipped `SqlEditor` header already cites its rubix-cube/Grafana
origins, so per-file attribution is the established pattern here).

## Open questions — RESOLVED (peer review 2026-07-06)

1. **CodeMirror vs Monaco** — **CONFIRMED by the user: CodeMirror.** Monaco stays the documented rejected
   alternative (~2 MB + worker + global-theme singleton vs the multi-pane Dockview). Revisit only if a
   post-ship gap proves unacceptable; the splitter + formatter ports carry over either way.
2. **Formatter keybinding** — DECIDED: yes, add `Cmd/Ctrl+Shift+F` alongside the button (cheap, expected
   muscle memory).
3. **Splitter home** — DECIDED: `ui/src/lib/sql/split/` (feature-neutral; it will serve surfaces beyond the
   builder). Copied verbatim with Apache-2.0 attribution headers — the full-copy discipline applies here at
   full strength (pure TS, zero coupling).

## Related

- [`query-builder-10x-scope.md`](query-builder-10x-scope.md) — the umbrella (the CodeMirror-vs-Monaco decision lives here as OQ #1).
- `ui/src/features/dashboard/builder/editors/SqlEditor.tsx` · `theme.ts` (`useCodeMirrorTheme`) — the editor + its theme, edited here.
- `ui/src/features/dashboard/builder/sql/{RawEditor,SqlQueryHeader}.tsx` — pass-through prop + Format button.
- `ui/src/lib/schema/schema.api.ts` (`Schema`) — the completion source shape.
- Tabularis reference (design + portable pure TS): `/tmp/tabularis/src/utils/sqlSplitter/*` (copy), `/tmp/tabularis/src/utils/{sqlFormat,autocomplete,identifiers,sqlAnalysis}.ts` (design), `/tmp/tabularis/src/hooks/useSqlAutocompleteRegistration.ts` (the Monaco path we deliberately do NOT take).
</content>
