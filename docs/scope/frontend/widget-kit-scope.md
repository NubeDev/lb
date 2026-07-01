# Frontend scope — the Widget Kit: one reusable widget + field-presentation library, with an author SDK

Status: **Phase 1 shipped** (2026-07-02) — field presentation (both sides) + input-widget extraction +
the shared table core; the reminder table + `/remind` form fixed. Phase 2 remains scope (named below).
Shipped truth: [`../../public/frontend/widget-kit.md`](../../public/frontend/widget-kit.md); working log:
[`../../sessions/frontend/widget-kit-session.md`](../../sessions/frontend/widget-kit-session.md). Topic:
`frontend`.

Today the platform already has a **string-keyed widget contract** (an `x-lb.widget` form vocabulary and an
`x-lb-render` response-view vocabulary, both resolved by string with an `ext:<id>/<widget>` federation
path — see `channels-rich-responses-scope.md`). But three things are missing to make widgets *actually*
reusable across the whole system, and the reminder table + `/remind` form make all three visible:

1. **No declarative per-field presentation.** A `reminder.list` table renders raw record keys as column
   headers (`maxRuns`, `nextAttemptTs`, `principalSub`) and dumps the nested `action` object as a JSON
   blob; the `reminder.create` form shows `action_kind`/`max_runs` verbatim. A field author has **no way**
   to say "call this **Max Runs**", "here's help text", or "**hide** this column" — even though the
   dashboard's `fieldConfig` already carries `displayName`/`description` (and applies them **only** to cell
   *formatting*, never to a table header or a form label).
2. **The widgets are physically trapped in feature folders.** The cron/select/number/date/boolean/text/sql
   input widgets live under `channel/palette/argWidgets/`; the visual cron builder lives under `reminders/`;
   the view renderers + controls live under `dashboard/views/`. "Reusable across the whole system" is
   aspirational — a non-palette surface (or an extension) can't import them cleanly, and the registry that
   ties them together is a palette-local file.
3. **The extension widget SDK can't author an input widget.** The shipped `RemoteWidgetMount(el, ctx,
   bridge, widgetId)` federation contract has **no value/onChange channel** (documented verbatim in
   `argWidgets/registry.ts`), so an `ext:` **form** widget can only fall back to a text box — it cannot
   report a collected value. And there's no packaged, versioned widget-authoring contract a third-party dev
   builds against.

This scope makes the widget system genuinely common: **(A) a declarative field-presentation vocabulary**
(`label`/`description`/`hide`/`order`) that both the request form and the response table honor through one
shared resolver; **(B) an extracted `lib/widgets/` library** — the registry, the built-in input widgets,
the shared field-presentation, and the mount/bridge types moved out of the dashboard/palette feature
folders into one internal package that palette, dashboard, channel responses, and extension surfaces all
consume; **(C) an author SDK** — a versioned mount contract with a value channel so extensions can ship
**input** widgets too, plus the widget-authoring types packaged for third-party devs.

## Goals

- **Declarative per-field presentation, backend-owned, applied everywhere.** A field author declares
  `label` (override the humanized name), `description` (help/tooltip), `hide` (omit from the surface), and
  `order` — once, on the descriptor — and **both** the form (request) and the table/view (response) render
  it identically. The camelCase→Title-Case humanize (`maxRuns` → "Max Runs") is the **fallback**, not the
  ceiling; a label override wins. This retires the raw-column-header problem for **every** command, not
  reminders alone.
- **One field-presentation resolver, shared by request and response.** Form-field hints ride `x-lb` on the
  schema property (beside `widget`/`showIf`); response/data-field hints reuse the shipped
  `fieldConfig.displayName`/`description` (Grafana-adopted) **plus a new `hide`** — and **both** resolve
  through one `resolveFieldPresentation()` + one `humanize()` in `lib/widgets`, so a header and a form label
  never drift. No second presentation stack (the same principle `fieldconfig/format.ts` holds for
  formatting).
- **Extract the widgets into a common `lib/widgets/` library.** Move the input widgets
  (`cron`/`select`/`number`/`boolean`/`date`/`text`/`sql`/`entity`/`runtime`), the visual `CronBuilder`, the
  registry (the public API), the shared field-presentation, and the mount/bridge/field types out of
  `channel/palette/argWidgets/`, `reminders/`, and `dashboard/` into **one internal package** with a small
  public surface. Palette, dashboard, channel `ResponseView`, and any future surface import the **same**
  widget by string — the registry becomes a library export, not a palette file.
- **De-duplicate the two table renderers.** `dashboard/views/table/TablePanel.tsx` (read-only) and
  `channel/ResponseTable.tsx` (row-controlled) both introspect columns from raw keys. Factor the
  column-model (presentation-resolved headers, hide, order, nested-value rendering) into one shared
  `lib/widgets` table core both consume; the only difference stays the per-row control column.
- **An author SDK with a value channel — extensions can ship INPUT widgets.** Version the mount contract so
  a widget can run in **input mode**: the context carries an initial `value` and an `onValue(next)`
  callback, letting an `ext:<id>/<widget>` form widget report a collected value back to the palette (closing
  the gap `registry.ts` documents). Package the widget-authoring types + a `defineWidget` helper so a
  third-party dev builds a widget against a **stable, versioned** contract — and declares it (view **or**
  input) in the `[[widget]]` manifest.
- **Additive and versioned — nothing shipped breaks.** Field-presentation keys are optional vendor
  extensions (`x-`-keyed → an off-the-shelf JSON-Schema validator ignores them); the mount-contract bump is
  additive (`ctx.v`); the library extraction is a **move + re-export**, not a rewrite. An older descriptor
  with no presentation hints renders exactly as today (humanize fallback).

## Non-goals

- **No new render engine, view, or trust tier.** This reuses the shipped v2 widget contract
  (`widget-builder-scope.md`) and the string-keyed registry (`channels-rich-responses-scope.md`). Adding a
  *new* view/widget kind is additive there, consumed here for free.
- **No new MCP verb, capability, or datastore.** Presentation hints ride the existing `input_schema` /
  `ToolDescriptor.result` / `fieldConfig` (already on descriptors/records). No `mcp:*` added, no table
  added. If a build finds it needs one, that's a finding to surface, not to sneak in.
- **No formatting semantics.** *Value* formatting (unit/decimals/thresholds/mappings/color) stays owned by
  `fieldconfig/` + the user-prefs bridge (`field-config-scope.md`). This scope owns **presentation of the
  field's identity** (name/description/visibility/order), not the value's format. The two compose.
- **No WIT/ABI change.** The plugin ABI (`sdk/wit/world.wit`, tool dispatch + host callback) has **no**
  widget types and gains none — widget authoring is a **frontend federation** contract (the `mount` seam),
  versioned independently of the WIT `world`. Flagged in "How it fits the core".
- **`hide` is presentation, never security.** A hidden field is omitted from a *rendered surface*; it is
  **not** an access control. The data still crosses the bridge under the viewer's grant; anything truly
  secret must be denied server-side, not hidden client-side. Called out as a risk + a test.
- **No big-bang move.** The extraction is phased (§ below): Phase 1 field-presentation + input-widget
  extraction + the shared table core; Phase 2 the view renderers/controls + the SDK value channel + ext
  input widgets. No `*.fake.ts` — real gateway, real installed reference widget.

**Skill doc (§6 checklist): N/A.** This scope adds **no** agent-/API-drivable verb — it's a UI contract
convention (field-presentation hints on existing descriptors), a frontend library extraction, and a
federation-context bump. The surfaces it touches (the palette command surface, `dashboard.save`,
`tools.catalog`) already own their skills; nothing here is a new automatable task. If Phase-2's `devkit`
widget-scaffold follow-up (named below) lands as a `devkit.*` verb, that session owns its skill — but none
is added here.

## Intent / approach

**The contract already exists; make it reusable and give it a presentation layer.** The platform decided
(rightly) that the UI is a *generic schema+render interpreter*: forms come from `input_schema` + `x-lb`
hints, responses from `descriptor.result` + a string-keyed view registry, and unknown widgets/views degrade
to a fallback. What's missing is (a) a per-field presentation vocabulary on that contract, (b) a physical
home for the widgets so "reusable" is real, and (c) a value channel so the *extension* story covers input,
not just read-only tiles.

The three moves, each small and additive:

- **(A) Field presentation = four optional keys + one resolver.** On a **form** field:
  `"maxRuns": { "type":"number", "x-lb": { "widget":"number", "label":"Max Runs", "description":"Stop
  after N fires (blank = forever)", "order": 5 } }`. On a **response/table** field: the shipped
  `fieldConfig` override `byName:"maxRuns"` gains `{ displayName:"Max Runs", description:"…", hide:false }`.
  Both are read by **one** `resolveFieldPresentation(fieldName, hints)` in `lib/widgets`, which returns
  `{ label, description, hidden, order }` with the `humanize(fieldName)` fallback for `label`. The palette
  arg-rail, the two table renderers, and any view that lists fields call it — so `maxRuns` is "Max Runs"
  everywhere, `principalSub`/`ts` can be hidden from the reminder table, and `action` can carry a
  `description` (or a nested-view hint) instead of a raw blob.
- **(B) `ui/src/lib/widgets/` — the extraction.** A new internal library folder (one responsibility per
  file, FILE-LAYOUT). Its public API is the **registry** (`resolveWidget` for inputs, plus the shared
  field-presentation and the mount/bridge/field types). Moves in: the input widgets + `CronBuilder`, the
  field-presentation + humanize, the shared table column-model, and the `WidgetKind`/`XLbHint`/mount types
  (relocated from `lib/channel/palette.types.ts` + `dashboard/builder`). `channel/palette` and `dashboard`
  keep their **controllers** (the arg-rail state machine; the grid/builder) but import the **widgets** from
  the library. The move is import-rewrites + re-exports — behavior-preserving, guarded by the existing
  palette/dashboard tests staying green.
- **(C) The author SDK — a value channel + packaged types.** Bump the federation mount context to carry an
  optional input channel: `ctx.mode: "view" | "input"`, `ctx.value?: unknown`, `ctx.onValue?: (v) => void`.
  A view widget ignores it (unchanged); an **input** widget reads `ctx.value` and calls `ctx.onValue` — so
  an `ext:<id>/<widget>` can be a real form field, not a text fallback. Package the widget types + a
  `defineWidget({ mount })` helper (the SDK the extension author builds against), and let `[[widget]]`
  declare `kind = "view" | "input"`. This is the request-side twin of the already-shipped read-side ext
  widget — the same federation/trust path, one field added to the context.

**Why one shared resolver instead of two presentation paths.** The alternative — let the table renderer keep
its raw-key headers and add a *separate* form-label mechanism — forks presentation into two places that
drift (a field is "Max Runs" in the form and "maxRuns" in the table, exactly today's split between
`fieldConfig.displayName` and the raw header). One `resolveFieldPresentation` + one `humanize`, consumed by
both sides, is the same "don't fork the formatter" discipline `field-config-scope.md` already holds for
values. **Rejected.**

**Why extract to a library instead of cross-importing between features.** The widgets currently live under
`channel/palette/argWidgets/` and `dashboard/`; a third surface (or an extension host) importing
`@/features/channel/palette/argWidgets/CronArg` couples unrelated features and violates the one-way
dependency direction. A `lib/widgets/` package with the registry as its public API is the honest home — it's
what "common reusable widgets across the whole system" *means*. **Rejected: leave them in place and
cross-import.**

**Why a value channel on the existing mount contract, not a new input-widget federation path.** A parallel
"ext input widget" mount API would double the federation/trust/bridge surface (the exact trap
`channels-rich-responses-scope.md` calls out). One additive field on the shipped context (`mode`+`value`
+`onValue`) reuses the whole trust router, the grant leash, and the iframe wall — a view widget that ignores
it is untouched. **Rejected: a second federation entry point for input widgets.**

**Rejected alternatives (whole-scope):**

- *Patch the reminder table's headers directly.* Fixes one table, leaves every other command with raw keys
  and no form labels — the same per-command trap the rich-responses scope removed for rendering. A generic
  presentation vocabulary retires the class.
- *Put presentation in a new MCP verb / a per-workspace prefs record.* Over-engineered — presentation is
  authored **with the field**, on the descriptor the field already ships on (`input_schema`/`result`/
  `fieldConfig`). A viewer-prefs *override* of presentation (like unit prefs) is a named follow-up, not the
  base contract.
- *Adopt a heavyweight third-party form library (RJSF etc.).* Rejected — the platform already has a
  string-keyed widget registry, a bridge, and a trust router; a form lib forks all three and can't express
  `ext:<id>/<widget>` federation or the grant leash. Extend the shipped contract.

## How it fits the core

- **Tenancy / isolation (rule 6):** presentation hints are **inert data** on the workspace-scoped descriptor
  / `dashboard:{id}` record — no key, no wall of their own. A widget still reaches data only through the
  host-mediated bridge, workspace derived from the **viewer's** token (never the descriptor or an iframe
  `postMessage`). Extracting the widgets to a library changes **no** data path — the same `makeWidgetBridge`
  leash applies. Mandatory two-session test: a ws-B viewer rendering a presentation-hinted form/table
  reaches only ws-B; an ext input widget's `onValue`→bridge call is ws-B-scoped.
- **Capabilities (rule 5/7):** **no new capability.** Presentation invents none — it re-labels/hides what a
  view already renders and a form already collects; the field's data still crosses the existing tool's cap ∩
  grant, host-re-checked. The deny path is unchanged and is a headline test: a **hidden** field on a table
  whose `source` the viewer lacks still shows the honest "no access" fallback (hiding is not a bypass, and a
  denied source is denied whether or not a field is hidden).
- **Placement (rule 1):** `either`. Presentation is pure data; the widget library is shell code that runs
  identically on edge and cloud, Tauri `invoke` or gateway SSE. No role branch.
- **MCP surface (§6.1):** **none added.** This scope is a **contract convention + a UI refactor + a
  federation-context bump** — it touches no verb. Field-presentation hints ride `tools.catalog`'s existing
  `input_schema`/`result` (a command already carries them) and `dashboard.save`'s existing `fieldConfig`.
  Get/list/live-feed/batch: all N/A — nothing new to read or write. (The one Rust-visible change is that a
  descriptor *author* may now emit the optional hint keys; the wire type is `Value`, already open.)
- **Data (SurrealDB):** no new table, no new record. `fieldConfig` gains an optional `hide: boolean` on
  `FieldOptions` (additive, `#[serde(default)]`, rides the existing `dashboard.save` UPSERT). Form hints
  live in `input_schema` (already a `Value` on the descriptor). State vs motion untouched.
- **Bus (Zenoh):** unchanged. No presentation motion; a widget's data/stream uses the existing bridge/SSE.
- **Sync / authority:** additive on the shipped `(table,id)` dashboard UPSERT and on the in-code descriptor;
  an older node ignores unknown hint keys (forward-compatible by serde / by the UI's unknown→humanize
  fallback). No new authority.
- **Secrets:** none reach the widget or its presentation; a field needing a secret pulls it server-side in
  the tool handler (unchanged). `hide` never touches a secret — secrets are denied, not hidden.
- **Stateless extensions (rule 4):** an ext input widget holds no durable state — its value flows through
  `onValue` into the palette's values object (submitted via the bridge), exactly as a built-in widget's
  value does. Nothing persists in the widget instance (hot-reload safe).
- **One responsibility per file (rule 8):** the library is one-widget-per-file (as `argWidgets/` already
  is); the resolver/humanize/table-core are each their own file. The extraction *improves* layout — it moves
  feature-trapped widgets to a shared home.
- **SDK/WIT impact — FLAG (the one boundary a reviewer signs off).** Two versioned frontend contracts move,
  **neither touches WIT**: (1) the **field-presentation vocabulary** — optional `label`/`description`/
  `hide`/`order` on `x-lb` (form) and `hide` on `fieldConfig` (response), both `x-`/vendor-safe and
  additive; (2) the **federation mount context** bumps `v` to carry `mode`/`value`/`onValue` for input
  widgets — additive, a view widget ignoring the new fields is unchanged. The **`[[widget]]` manifest**
  gains an optional `kind = "view" | "input"` (default `view`). The plugin **WIT ABI is untouched** (it has
  no widget types); widget authoring is a frontend federation contract versioned on `ctx.v`, not the WIT
  `world`. The packaged author types (`defineWidget`, the context/bridge/field types) are the stable surface
  a third-party dev depends on — freeze the shape here.

## Example flow

1. **The reminder table, fixed generically.** `reminder.list`'s `descriptor.result` (a `table` view) — or
   its `fieldConfig` overrides — declares presentation: `maxRuns → {label:"Max Runs"}`, `nextAttemptTs →
   {label:"Next fire", ...}`, `principalSub → {hide:true}`, `ts → {hide:true}`, `action → {label:"Action",
   description:"What fires"}`. The shared table core calls `resolveFieldPresentation` per column: headers
   read "Max Runs / Next fire / Action", `principalSub`+`ts` drop, and `action` renders through the nested
   presentation instead of a raw JSON blob. **Zero table-component change per command** — the descriptor
   carries it.
2. **The `/remind` form, same vocabulary.** The `reminder.create` `input_schema` adds `label`/`description`
   beside the `x-lb.widget` hints (`action_kind → "Action Kind"`, `max_runs → "Max Runs", description:"Stop
   after N fires"`). The palette arg-rail renders the override label + a help tooltip — the same
   `resolveFieldPresentation` the table used, so form and table agree.
3. **A widget imported by three surfaces.** The `cron` widget now lives in `lib/widgets`. The palette
   imports it for `/remind`; the dashboard imports it for a hypothetical schedule control; a channel
   `ResponseView` imports it — all via `resolveWidget("cron")`, one implementation, no duplication.
4. **An extension ships an INPUT widget.** A `color-scheme` extension declares `[[widget]] kind="input"
   id="swatch"`. A command's schema references `x-lb:{ widget:"ext:color-scheme/swatch" }`. The palette
   mounts it in **input mode** (`ctx.mode="input"`, `ctx.value=current`, `ctx.onValue=setValue`); the widget
   reports the picked color back — a real form field, not the text fallback. Trust/leash identical to the
   read-side ext widget (iframe for untrusted, `[[widget]].scope ∩ grant`).
5. **Graceful degradation, unchanged.** A descriptor emits `x-lb:{widget:"color-picker"}` an older UI
   doesn't know → text fallback. A field with no presentation hints → `humanize` ("maxRuns" → "Max Runs").
   A `hide` an old renderer ignores → the column shows (honest, not a crash). Additive + versioned.
6. **Hiding is not security.** A table hides `principalSub`; a curious viewer inspecting the network sees the
   `reminder.list` row still contains it (it crossed under their grant). Nothing secret was "hidden" —
   secrets are denied server-side. The deny test asserts a truly-ungranted source is denied whether or not a
   field is marked `hide`.

## Testing plan

Per `scope/testing/testing-scope.md` — real gateway, real seeded rows, a **real installed reference
widget/extension**; **no `*.fake.ts`**.

- **Field presentation applied on BOTH sides (the headline):** a seeded command whose `input_schema` +
  `result`/`fieldConfig` carry `label`/`description`/`hide`/`order` → assert the **form** renders the
  override labels + descriptions and the **table** renders the same headers, drops the hidden columns, and
  orders as declared. A field with **no** hints falls back to `humanize` (a unit test: `maxRuns` → "Max
  Runs", `nextAttemptTs` → "Next Attempt Ts") — one resolver, asserted from both call sites.
- **Reminder regression (the motivating case, real gateway):** the `/reminders` table renders "Max Runs"
  (not `maxRuns`), hides `principalSub`/`ts`, and does not dump `action` as a raw blob — asserted through
  the real `ResponseTable` mount over a spawned node (extends the shipped reminders gateway test).
- **One widget, many surfaces (no duplication):** a test that `resolveWidget("cron")` returns the **same**
  `lib/widgets` implementation imported by the palette and by a dashboard mount — and a repo check (grep/
  lint) that the input widgets no longer live under `channel/palette/argWidgets` or `reminders` (they moved,
  not copied). The existing palette + dashboard suites stay green across the move (behavior-preserving).
- **Ext INPUT widget reports a value (real installed widget):** install a reference extension declaring
  `[[widget]] kind="input"`; a command referencing `ext:<id>/<widget>` mounts it in input mode; the widget's
  `onValue` drives a real submit whose bridged args carry the collected value — over the real gateway, host
  effect asserted. The untrusted variant mounts in the **iframe** and reports its value over the
  `postMessage` bridge (trust router unchanged).
- **Capability deny (mandatory):** a table/form whose `source`/`action` names a tool outside the viewer's
  grant is denied **server-side** — and a **hidden** field does not change that (deny is opaque with or
  without `hide`; hiding is presentation, not a gate). The ext input widget's `onValue`→bridge call is
  leashed to `[[widget]].scope ∩ grant`, re-checked at the host.
- **Workspace isolation (mandatory):** two real sessions — a ws-B viewer renders a presentation-hinted
  form/table and an ext input widget; every bridged call is ws-B (from the token), never ws-A; presentation
  hints on a ws-A descriptor are never resolved against ws-B data.
- **Token never crosses:** the session token appears in no in-process bridge arg nor iframe `postMessage`
  (input mode included — the `onValue` path carries a value, never the token). Reuse the shipped assertion
  on the new input channel.
- **Versioning / degradation:** an unknown `x-lb.widget`, an unknown presentation key, and an unknown
  `ctx.v` field each degrade honestly (text fallback / humanize / ignored) — no crash, old descriptors
  unaffected.

## Risks & hard problems

- **`hide` reads as security but isn't (load-bearing).** The single most likely misuse: an author "hides"
  `principalSub`/a token/a secret and assumes it's protected. It is **presentation only** — the data crossed
  the bridge under the viewer's grant. The doc, the type's doc-comment, and a test must all state it, and
  anything secret must be denied server-side. If `hide` ever becomes the reason a field is "safe", that's a
  vulnerability.
- **The extraction is a wide, behavior-preserving move — regressions hide in imports.** Moving widgets out of
  `channel/palette/argWidgets` + `dashboard` touches many import sites. The guardrail: the move is
  re-export-first (old paths re-export from `lib/widgets` during a transition), the existing palette +
  dashboard suites must stay green with **no** assertion changes, and a follow-up removes the shims. A move
  that needs test edits is a behavior change in disguise — surface it.
- **Two declaration sites (form `x-lb` vs response `fieldConfig`) could still drift.** The mitigation is the
  **single resolver** — both sites must funnel through `resolveFieldPresentation`/`humanize`; a renderer
  that hand-labels a header outside it forks presentation again (the exact `field-config-scope.md` lesson).
  Lint the call sites.
- **The mount-context bump is a forever contract for third parties.** Once an extension author ships against
  `ctx.mode`/`value`/`onValue` + `defineWidget`, the shape is expensive to change. Get the input-mode
  contract right once (a value channel + an initial value + a version stamp), matching how the read-side
  context is already versioned. Freeze it here.
- **Nested/complex fields (the `action` blob).** A field whose value is an object (`action`) needs more than
  a label — either a nested presentation (render sub-fields) or an explicit "render via view X" hint.
  Phase 1 ships label+description+hide (turns the blob into a labeled, describable cell / hides it); a
  **nested-field view** is a named follow-up, not a Phase-1 blocker.
- **Ordering vs schema order.** `order` competing with the schema's own property order + the `required`-first
  arg-rail order can confuse. Decide: `order` is an **optional** override; absent → today's order (required
  first, then schema order). Documented, not implicit.

## Open questions

Decisions to take during implementation; residuals are named follow-ups.

**Proposed decisions (take unless a reviewer objects):**

- **Field-presentation keys:** `label`, `description`, `hide`, `order` — on `x-lb` (form) and, for response
  fields, on `fieldConfig` `FieldOptions` (`displayName` already = `label`; add `hide`). One resolver reads
  both. Keep them optional + vendor-safe.
- **Library home:** `ui/src/lib/widgets/` (beside `lib/channel`, `lib/dashboard`), registry as the public
  API. Input widgets + `CronBuilder` + field-presentation + table-core + mount/field types move in; the
  palette arg-rail controller and the dashboard grid/builder stay in their features and import the library.
- **Input-widget contract:** additive `ctx.mode`/`value`/`onValue` on the versioned federation context;
  `[[widget]] kind = "view" | "input"` (default `view`). Package `defineWidget` + the types as the author
  SDK surface (co-located with the `lb-devkit` ui template so a scaffolded widget compiles).
- **Phasing:** Phase 1 — field presentation (both sides) + input-widget extraction + shared table core (the
  reminder table fixed). Phase 2 — move the view renderers/controls into the library + the SDK value channel
  + ext input widgets. Ship Phase 1 first; it retires the raw-header/label complaint immediately.
  **✓ Phase 1 SHIPPED (2026-07-02)** — all Phase-1 decisions above taken as written. One contract touch
  surfaced + accepted (in-scope, no new verb): the rich_result render envelope (`RichResultPayload`, TS +
  Rust) gained an optional `fieldConfig` so the descriptor-declared table presentation reaches
  `ResponseTable` via `buildCell` → `cell.fieldConfig` — additive data on the existing envelope, mirroring
  how `dashboard.save` already carries `fieldConfig`. Phase 2 remains open.

**Named follow-ups (not Phase-1 blockers):**

1. **Viewer-prefs override of presentation** — a viewer relabeling/reordering columns locally (like unit
   prefs override formatting). Additive over the resolver; a prefs concern, not the base contract.
2. **Nested-field presentation** — declaring how an object field (`action`) renders its sub-fields (a mini
   view), beyond label/hide. Additive vocabulary.
3. **Move the view renderers wholesale** — Phase 2 completes the library by relocating `dashboard/views/*`
   controls/renderers so the dashboard, too, imports from `lib/widgets`. Bounded by the same
   re-export-first discipline.
4. **Grouped/sectioned forms** — presentation `group`/section hints for multi-field forms (the
   schema-driven form renderer named in the `/remind` handover consumes the same presentation layer).
5. **A `devkit` widget scaffold template** — `devkit.scaffold` gains a `widget` feature that emits a
   `defineWidget` stub (view or input) wired to the bridge, so "generate a widget" is one wizard step
   (`ext-sdk-scope.md`).

## Related

- `scope/channels/channels-rich-responses-scope.md` — the **string-keyed widget/view contract** this builds
  on (the `x-lb.widget` form vocabulary, the `x-lb-render` response envelope, `ext:<id>/<widget>` federation,
  unknown→fallback). This scope adds the **presentation layer** and the **physical library** that contract
  implied.
- `scope/channels/channels-command-palette-scope.md` — the arg-rail (request surface) that consumes the
  extracted input widgets + the new form-field presentation.
- `scope/frontend/dashboard/widget-builder-scope.md` — the **shipped v2 widget contract** (`mount(el, ctx,
  bridge)`, trust tiers, the bridge leash) whose **context this scope versions** to add the input channel.
- `scope/frontend/dashboard/viz/field-config-scope.md` — owns `FieldOptions` (`displayName`/`description`);
  this scope adds `hide`, **applies** them to table headers/form labels, and shares the resolver — it
  **consumes** the formatting side, never re-implements it.
- `scope/extensions/ui-federation-scope.md` — the federated-widget mount + iframe/trust router the input
  channel extends; `scope/extensions/ext-sdk-scope.md` — the `lb-devkit` path the packaged author types +
  the widget scaffold template plug into.
- `scope/frontend/ui-standards-scope.md` — the shadcn-first primitives every extracted widget must obey.
- `docs/debugging/channels/palette-conditional-required-fields-unreachable.md` +
  `docs/sessions/reminders/remind-form-conditional-fields-session.md` — the `/remind` slice that surfaced
  the label/hide/description gap this scope generalizes (`x-lb.showIf`/`requiredWhenShown` are the
  conditional twin of these presentation keys).
- README **§3** (rules 4/5/6/7/8), **§6.1** (API shape — why no verb is added), **§6.13** (extension UIs —
  federation vs iframe by trust), **§7** (tenancy).

### One-responsibility-per-file plan (FILE-LAYOUT)

Phase 1 lands under `ui/src/lib/widgets/`:

- `registry.ts` — the public widget resolver (moved from `channel/palette/argWidgets/registry.ts`), the
  library's API.
- `inputs/` — one file per input widget (`CronArg`/`SelectArg`/`NumberArg`/`BooleanArg`/`DateArg`/`TextArg`/
  `SqlArg`/`EntityArg`/`RuntimeArg`) + `CronBuilder` (moved from `reminders/`).
- `presentation/resolve.ts` — `resolveFieldPresentation(fieldName, hints) → {label,description,hidden,order}`.
- `presentation/humanize.ts` — `humanize("maxRuns") → "Max Runs"` (the fallback).
- `table/columns.ts` — the shared column-model (presentation-resolved headers, hide, order, nested value)
  both `TablePanel` and `ResponseTable` consume.
- `types.ts` — `WidgetKind`/`XLbHint` (relocated from `lib/channel/palette.types.ts`) + the mount/context/
  bridge/field types.

The palette arg-rail controller (`CommandPalette.tsx`) and the dashboard grid/builder stay in their
features and **import** the library. Phase 2 relocates `dashboard/views/*` renderers/controls and adds the
`ctx.mode`/`value`/`onValue` input channel + `defineWidget`.
