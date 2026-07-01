# Widget Kit — Phase 1: field-presentation + input-widget extraction + shared table core (session)

- Date: 2026-07-02
- Scope: ../../scope/frontend/widget-kit-scope.md
- Stage: S8 shipped — building on the real workspace (STATUS.md)
- Status: done

## Goal
Ship Phase 1 of the Widget Kit scope: (A) a declarative per-field presentation vocabulary
(`label`/`description`/`hide`/`order`) applied on BOTH the request form and the response table
through ONE resolver; (B) extract the input widgets + registry into `ui/src/lib/widgets/`;
(C) a shared table column-model both table renderers consume. The motivating fix: the
`/reminders` table renders "Max Runs" (not `maxRuns`), hides `principalSub`/`ts`, and does not
dump `action` as a raw JSON blob — green over the REAL gateway. Phase 2 items (view renderers,
ext value channel, ext input widgets) are OUT.

## What changed

**(A) Declarative per-field presentation, one resolver, both surfaces.**
- `lib/widgets/types.ts` — `XLbHint` gains `label`/`description`/`hide`/`order` (form site); a new
  `FieldPresentationHints` (accepts `displayName` as the `fieldConfig` alias for `label`) + the resolved
  `FieldPresentation` (`{label, description, hidden, order}`) — its doc-comment states `hidden` is
  presentation, NOT security.
- `lib/widgets/presentation/humanize.ts` — `humanize(name)` (camel/snake/kebab/acronym → Title Case).
- `lib/widgets/presentation/resolve.ts` — the ONE `resolveFieldPresentation(name, hints)`; label override
  (`label`/`displayName`) wins, else humanize; `hidden` from `hide`; `order` passthrough (optional).
- `lib/dashboard/fieldconfig.types.ts` — `FieldOptions` gains `hide?` + `order?` (additive; doc-comment:
  presentation, not security).

**(B) The `lib/widgets/` library (behavior-preserving MOVE).** See the extraction map. Old paths are
re-export shims (`argWidgets/registry.ts`, `features/reminders/CronBuilder.tsx`); `ActiveArgWidget`
(a palette controller, stays in the feature) now imports the leaf widgets from `@/lib/widgets/inputs`.

**(C) Shared table column-model.** `lib/widgets/table/columns.ts` — `columnsOf`, `resolveColumns(rows,
fieldConfig)` (headers via the ONE resolver, drops `hide`, applies `order`, first-seen otherwise), and
`cellText` (nested object → readable JSON, not a thrown blob). BOTH `TablePanel` (read-only) and
`ResponseTable` (row-controlled) now consume it; ResponseTable's only extra stays the per-row control column.

**Wiring for the motivating fix (fieldConfig → table).**
- `RichResultPayload` (TS) + `RichResultPayload` (Rust `channel/payload.rs`) gain an optional
  `fieldConfig` — inert data on the EXISTING envelope, NO new verb/table.
- `ResponseView.buildCell` copies `payload.fieldConfig` onto `cell.fieldConfig`.
- `reminder/descriptor.rs`: `create_schema` fields carry `label`/`description` on `x-lb`;
  `list_render()` declares a `fieldConfig` (`maxRuns`→"Max Runs", `nextAttemptTs`→"Next fire",
  `action`→"Action"/"What fires", `principalSub`+`ts` hidden). Two new Rust unit tests assert both.
- `CommandPalette` renders the resolved label + description above the active arg widget (the SAME resolver
  the table headers use — form and table can't drift).

### The `fieldConfig`-plumbing finding (surfaced, not snuck in)
The `RichResultPayload` render envelope carried NO `fieldConfig`, and `buildCell` never set
`cell.fieldConfig` — so `ResponseTable` rendered raw headers regardless of any presentation. `Cell.fieldConfig`
already existed (TablePanel read it). Threading `fieldConfig` through the envelope → cell is additive DATA on
the shipped envelope (mirrors how `dashboard.save` already carries `fieldConfig`), NOT a new MCP verb,
capability, WIT/ABI, or table — so it is in-scope per the scope's "no new verb" rule. Recorded here as the
one contract touch a reviewer signs off.

### Extraction map (behavior-preserving MOVE, re-export from old paths)

| From | To |
|---|---|
| `features/channel/palette/argWidgets/registry.ts` | `lib/widgets/registry.ts` |
| `features/channel/palette/argWidgets/{Cron,Select,Number,Boolean,Date,Text,Sql,Runtime,Ext}Arg.tsx`, `EntityPicker.tsx`, `ActiveArgWidget.tsx`, `useRuntimes.ts`, `useSqlSchema.ts` | `lib/widgets/inputs/` |
| `features/reminders/CronBuilder.tsx` | `lib/widgets/inputs/CronBuilder.tsx` |
| `WidgetKind`/`XLbHint`/`EntityKind`/`SchemaProperty` in `lib/channel/palette.types.ts` | `lib/widgets/types.ts` |
| (new) | `lib/widgets/presentation/humanize.ts` |
| (new) | `lib/widgets/presentation/resolve.ts` |
| (new) | `lib/widgets/table/columns.ts` |

Old paths become re-export shims so every existing import + test stays green with NO assertion
changes (extraction discipline: a move that needs a test edit is a behavior change — stop).

## Decisions & alternatives
- **Response-side presentation rides `fieldConfig` (reuse, not a second stack).** The scope's
  decision: `fieldConfig.displayName` already == label; ADD an optional `hide`. Both form (`x-lb`)
  and table (`fieldConfig`) funnel through the ONE `resolveFieldPresentation` + `humanize`. Rejected
  a separate per-field block on the `x-lb-render` envelope — it forks presentation into two drifting
  places (the exact thing the scope forbids).
- **`fieldConfig` threaded onto the rich_result envelope.** FINDING while wiring: the
  `RichResultPayload` render envelope carried no `fieldConfig`, and `buildCell` never set
  `cell.fieldConfig` — so `ResponseTable` rendered raw headers. `Cell.fieldConfig` already exists
  (TablePanel reads it). Fix: add optional `fieldConfig?: FieldConfig` to `RichResultPayload`
  (additive data, NO new verb — it rides the existing envelope, mirroring how `dashboard.save`
  already carries `fieldConfig`), copy it in `buildCell`, and have `ResponseTable` resolve columns
  through it. This is the long-term-correct path (one resolver, one fieldConfig), not a reminder patch.
- **`order` is an optional override.** Absent → today's order (required-first for the arg rail,
  schema/first-seen order for tables). Never reorder implicitly.

## Tests

Mandatory categories covered: **capability-deny** (a viewer with list-only caps: the list command renders
but `reminder.update`/`reminder.create` are denied server-side — and the `hide` on the render changes
NOTHING about the deny; hiding is presentation, not a gate) and **workspace-isolation** (ws-B sees only its
own reminders, every bridged call is ws-B). Plus the scope's headline tests:

- **humanize unit** (`lib/widgets/presentation/humanize.test.ts`): `maxRuns`→"Max Runs",
  `nextAttemptTs`→"Next Attempt Ts", snake/kebab/acronym.
- **one resolver, both sites** (`presentation/resolve.test.ts`): `label` (form) and `displayName`
  (fieldConfig) resolve identically; unhinted → humanize; `hide`→hidden; `order` optional.
- **shared column-model** (`table/columns.test.ts`): headers via the resolver, hidden dropped, `order`
  reorders only when declared (else first-seen preserved), nested value → JSON.
- **one widget, many surfaces** (`lib/widgets/oneWidgetManySurfaces.test.ts`): `resolveWidget("cron")` is
  the one library entry; the barrel re-exports the identical component; a repo readdir asserts the leaf
  input widgets no longer live under `argWidgets` (moved, not copied).
- **reminder presentation regression over the REAL gateway** (extended
  `CommandPalette.reminders.gateway.test.tsx`): mounts the list render pulled off the LIVE `tools.catalog`
  (so the Rust `list_render()` fieldConfig is proven end to end) — headers read "Max Runs"/"Next
  fire"/"Action", `principalSub`/`ts` are dropped, no raw `action` blob; and `hide` is NOT security (the
  raw `reminder.list` row still carries `principalSub`). The `/remind` form shows the resolved "Channel"
  label + its description (same resolver as the table).
- **behavior-preserving move**: the existing palette + dashboard suites stayed green with NO assertion
  changes (the moved widget tests run at their new lib paths unchanged).

Green output:

```
$ cd rust && cargo test -p lb-host --lib
test result: ok. 83 passed; 0 failed; 0 ignored
  (incl. reminder::descriptor::create_schema_carries_form_presentation_labels,
         reminder::descriptor::list_render_declares_table_field_presentation,
         channel::payload::* 13 passed)
$ cargo fmt --check   # clean

$ cd ui && pnpm test
 Test Files  47 passed (47)
      Tests  313 passed (313)

$ cd ui && pnpm test:gateway src/features/channel/palette/CommandPalette.reminders.gateway.test.tsx
 Test Files  1 passed (1)
      Tests  11 passed (11)
  (incl. "presentation regression (the motivating fix): /reminders table shows 'Max Runs',
          hides principalSub/ts, no raw action blob")
```

## Debugging
None — no runtime bug (the one surprise, the missing `fieldConfig` on the envelope, was a wiring GAP found
while building, recorded above under "the fieldConfig-plumbing finding", not a regression). No
`docs/debugging/` entry needed.

## Public
Promoted to `docs/public/frontend/widget-kit.md` (Phase 1). Scope's Phase-1 status marked shipped;
STATUS.md updated.

## Scope open questions
The scope's "Proposed decisions" for Phase 1 are all taken as written (field-presentation keys, library
home, phasing). Phase-2 items (view-renderer move, the `ctx.mode`/`value`/`onValue` input channel,
`defineWidget`, ext input widgets) remain named follow-ups — explicitly OUT of this slice.
