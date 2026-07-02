# Widget Kit — reusable widgets + field presentation

Status: **Phase 1 shipped** (field presentation + input-widget extraction + shared table core). The ask +
full design live in [`../../scope/frontend/widget-kit-scope.md`](../../scope/frontend/widget-kit-scope.md);
the working log is [`../../sessions/frontend/widget-kit-session.md`](../../sessions/frontend/widget-kit-session.md).

## What shipped (Phase 1)

### Declarative per-field presentation — one resolver, both surfaces
A field author declares a field's **identity** — `label`, `description`, `hide`, `order` — once, and BOTH
the request form and the response table render it identically:

- **Form fields:** on `x-lb` (beside `widget`/`showIf`) in a command's `input_schema` —
  `"max_runs": { "type":"number", "x-lb": { "label":"Max Runs", "description":"Stop after N fires" } }`.
- **Response/table fields:** on the shipped `fieldConfig` `FieldOptions` — `displayName` already **==**
  label; Phase 1 **adds `hide`** (and `order`), additive (`#[serde(default)]`, rides `dashboard.save`, no
  new verb).

Both resolve through the ONE `resolveFieldPresentation(fieldName, hints)` in `lib/widgets` →
`{ label, description, hidden, order }`, with `humanize(fieldName)` as the label **fallback** (`maxRuns` →
"Max Runs", `nextAttemptTs` → "Next Attempt Ts"). A label override always wins. `order` is an **optional**
override — absent → the surface's natural order (required-first for the arg rail, first-seen for tables);
it never reorders implicitly. Every surface (palette arg rail + BOTH table renderers) funnels through this
one resolver — a header and a form label cannot drift.

**`hide` is presentation, NOT security.** A hidden field was still returned by the tool and crossed the
bridge under the **viewer's** grant — hiding removes it from a rendered surface, it does not gate access.
Anything truly secret must be **denied server-side** (a denied source is denied whether or not a field is
hidden); secrets are never merely hidden. Stated in the types' doc-comments and proven by a test.

### The `ui/src/lib/widgets/` library
The reusable widget system now lives in one internal package (was trapped in `channel/palette/argWidgets/`,
`features/reminders/`, and `dashboard/`):

```
lib/widgets/
  registry.ts              the public widget resolver (resolveWidget) — the library's API
  types.ts                 WidgetKind/XLbHint (+ presentation keys) + FieldPresentation types
  inputs/                  one file per input widget: Cron/Select/Number/Boolean/Date/Text/Sql/Runtime/Ext
                           + CronBuilder (moved from features/reminders/)
  presentation/humanize.ts humanize(name) — the label fallback
  presentation/resolve.ts  resolveFieldPresentation — the ONE resolver
  table/columns.ts         the shared column-model (resolved headers, hide, order, nested value)
```

The extraction was a **behavior-preserving move + re-export**: old import paths
(`channel/palette/argWidgets/registry`, `features/reminders/CronBuilder`) are thin re-export shims, so
nothing that imported them broke. The palette **arg-rail controller** (`CommandPalette.tsx`,
`ActiveArgWidget.tsx`, `EntityPicker.tsx`) and the dashboard grid/builder stay in their features and
**import** the library. The existing palette + dashboard test suites stayed green with no assertion changes.

### One shared table column-model
`lib/widgets/table/columns.ts` (`columnsOf` / `resolveColumns` / `cellText`) is consumed by BOTH
`dashboard/views/table/TablePanel.tsx` (read-only) and `channel/ResponseTable.tsx` (row-controlled). It
resolves headers through the presentation resolver, drops `hide`-marked columns, applies `order`, and
renders a nested object value as readable JSON (not a thrown blob). ResponseTable's only extra is its
per-row control column; TablePanel keeps its numeric value formatting (`fieldconfig/format`).

### The motivating fix (green over the real gateway)
`reminder.list`'s descriptor (`rust/crates/host/src/reminder/descriptor.rs`) declares a `fieldConfig` on its
`list_render()` envelope: `maxRuns` → "Max Runs", `nextAttemptTs` → "Next fire", `action` →
"Action"/"What fires", `principalSub` + `ts` **hidden**. The render envelope (`RichResultPayload`) gained an
optional `fieldConfig` (inert data on the existing envelope — no new verb/table), `ResponseView.buildCell`
copies it onto the cell, and the shared column-model resolves it. The `/reminders` table now reads author
labels, drops the hidden columns, and never dumps `action` as a raw blob. The `/remind` form shows the same
resolved labels + descriptions from the same resolver. Proven end to end by mounting the list render pulled
off the **live `tools.catalog`** in `CommandPalette.reminders.gateway.test.tsx`.

## No new contract
No new MCP verb, capability, datastore/table, or WIT/ABI change. Presentation hints ride the existing
`input_schema` / `fieldConfig` on descriptors and dashboard records; `fieldConfig` on the rich_result
envelope is additive data. An older node/UI degrades honestly: an unknown hint key is ignored, a field with
no hints humanizes.

## Not in Phase 1 (named follow-ups)
Phase 2 (the scope's "Phasing" decision): move the dashboard **view renderers/controls** wholesale into the
library; the federation **mount-context input channel** (`ctx.mode`/`value`/`onValue`) so extensions can
ship **input** widgets; the packaged `defineWidget` author SDK + `[[widget]] kind = "view" | "input"`.
Also: viewer-prefs override of presentation, nested-field presentation, grouped/sectioned forms, and a
`devkit` widget scaffold. None are shipped here.
