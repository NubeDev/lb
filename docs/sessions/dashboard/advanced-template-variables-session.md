# Session — advanced template variables (Grafana-parity depth)

Scope: [`docs/scope/frontend/dashboard/dashboard-variables-advanced-scope.md`](../../scope/frontend/dashboard/dashboard-variables-advanced-scope.md).
Deepens the shipped variable system + the frozen `vars` lib (`ui/src/lib/vars/`) — **additive only**, no
`VARS_LIB_V` bump (the resolved `VarScope` shape is unchanged: still `values` + `builtins`).

## What shipped

Held the "one model, one resolver" doctrine — every new capability is a field on the one `Variable` +
behavior in the one resolver/interpolator, no per-type code path.

### Frozen lib (`ui/src/lib/vars/`) — additive

- **`types.ts`** — new optional fields on `Variable`: `options?: VariableOption[]` (`{text,value}`),
  `allValue`, `regex` + `regexApplyTo`, `sort`, `refresh`, `hide`. New `datasource` variable type (still
  resolves through the one `{tool,args}` path, fixed `datasource.list` tool). Widened `FormatHint`:
  `regex`/`glob`/`percentencode`/`sqlstring`/`distributed`. `VARS_LIB_V` stays **1**.
- **`interpolate.ts`** — `formatValue` gains the new hint cases (regex-safe alternation with escaping,
  glob alternation, URL-encode, SQL single-quote-doubling).
- **`parseCustom.ts`** — `label : value` custom-string split (first unescaped ` : `, `\:` literal).
- **`regexOptions.ts`** — `applyRegex`: filters non-matching rows, splits `text`≠`value` on
  `(?<text>)`/`(?<value>)` named capture groups; **invalid regex fails honestly** (returns options
  unchanged, drops nothing); **ReDoS bounded** (pattern + input length caps).
- **`sortOptions.ts`** — Grafana sort modes over the display text (alpha/num, asc/desc, case-insensitive).
- **`depGraph.ts`** — chained/dependent variables: `orderVariables` topo-orders by `$var` references
  (derived, no explicit `dependsOn` edge); **a cycle throws `VarCycleError` (never hangs)**; `dependentsOf`
  gives the bounded downstream re-resolve set.

### Editor UX redesign (`ui/src/features/dashboard/vars/`)

The flat "wall of controls" row (name/label/type + 3 checkboxes + source + regex + 2 sorts + refresh +
hide, all on one plane) was replaced with a **type picker + sectioned form**:

- **`variableTypeMeta.ts`** (new) — the type catalog: icon + title + one-line description per
  `VariableType`. One source of truth; a new type is added here, not branched on.
- **`VariableTypePicker.tsx`** (new) — the "choose a variable type" step (Grafana-parity): a scannable
  list (icon tile + title + description), shown as the empty state and on "Add variable". Picking a type
  creates the variable and expands it.
- **`VariableRow.tsx`** (new) — each variable is now a row that is **collapsed to a summary line**
  (`$site · Query · multi`, with type icon + reorder/delete) or **expanded** (one at a time) into labelled
  sections: **General** (name/label/type + the type's description), **Values/Source** (type-specific — a
  custom variable shows a `Values` box teaching the `label : value` syntax, a query shows the source
  picker, a datasource shows a note), **Selection** (multi/all/required), and a disclosure-gated
  **Advanced**.
- **`variableSummary.ts`** (new) — the collapsed-row summary string.
- **`VariableAdvancedFields.tsx`** — reworked from a flat wrap into labelled fields (Regex / Apply-to /
  Sort / Refresh / Custom-All / Show-on-bar) inside the Advanced disclosure.
- **`VariableEditor.tsx`** — slimmed to orchestration: picker ↔ list, expand-one-at-a-time state, create
  of the right empty value shape per type.

Verified live in a real browser (Playwright against the dev server): the picker, the expanded query row,
the Advanced disclosure, and a two-variable list (one collapsed summary + one expanded) all render on-brand
with clear hierarchy. Icon buttons use the shadcn `Button`; the two large custom clickable surfaces (the
disclosure header, the type card) carry a scoped `no-restricted-syntax` disable. Lint + typecheck clean.

### Per-variable bar icon

- `Variable.icon?: string` (opaque icon-lib name) — additive/optional.
- Authoring: an **Icon** field at the head of the General section (`VariableRow`) — a compact trigger
  toggling an INLINE `IconPicker` (in-flow, not absolute, so the scrolling/overflow-hidden Sheet can't
  clip it) + a clear button. Reuses the shipped `@/lib/icons` (`Icon` + `IconPicker`).
- Render: `VariableBar` paints `<Icon name={variable.icon}>` before the label (a shared `VariableLabel`),
  honoring `hideLabel` (icon still shows).

### Host persistence (the closed-struct fix — important)

The Rust `dashboard::Variable` (`crates/host/src/dashboard/model.rs`) is a **closed struct**: `dashboard.save`
deserializes into it and re-serializes to the store, so any field NOT on the struct is **silently dropped**.
Every advanced field (icon, options, allValue, regex, regexApplyTo, sort, refresh, hide) was being lost on
save — the feature looked done in the UI but didn't persist. Fixed by adding all of them as additive
`#[serde(default, deserialize_with = "null_default")]` fields (matching `label`/`required`). Regression:
`variable_round_trips_advanced_fields` + `variable_tolerates_pre_advanced_shape` in `model.rs` (green).
NOTE: the dev node does not hot-reload Rust — a `make kill && make dev` (or node restart) is required before
the live save path carries these fields.

### Feature layer (`ui/src/features/dashboard/vars/`)

- **`resolveOptions.ts`** — `rowsToOptions` now yields `{text,value}` (honors `__text`/`__value`);
  `processOptions` runs the regex→sort pipeline; `staticOptions` honors `options`/`label : value`.
  **Bug fix (see debugging):** `extractRows` now reads the `series`/`samples`/`datasources` wrapper keys,
  not just `rows` — `series.find`/`.list` query variables never resolved options before.
- **`useVariableOptions.ts`** — interpolates the resolved `scope` into a variable's resolver args before
  the bridge call (the **chained-resolution seam**); a dependency's selection change re-keys the effect so
  the dependent re-resolves. Runs `processOptions` on the result.
- **`VariableBar.tsx`** — threads `scope` through; honors `hide` (`hideLabel`/`hideVariable`) and
  `allValue` (emitted verbatim on All instead of expanding every option).
- **`useVarScope.ts`** — the `$__all` sentinel resolves to a custom `allValue` when set.
- **`VariableAdvancedFields.tsx`** (new) — authoring controls for regex/sort/refresh/allValue/hide
  (split out of `VariableEditor` per FILE-LAYOUT — the editor was near the line budget).
- **`VariableEditor.tsx`** — adds the `datasource` type (seeds its fixed `datasource.list` resolver) and
  mounts the advanced-fields panel.

## Tests (green)

- Unit `lib/vars/advanced.test.ts` (21) — label/value split, regex filter+capture, sort variants, topo
  order + honest cycle, `dependentsOf`, new format hints. Frozen `vars.test.ts` (29) untouched → still green.
- Unit `resolveOptions.test.ts` — `{text,value}` shapes, `series` extraction, `__text`/`__value`.
- **Gateway `variablesAdvanced.gateway.test.tsx`** (real store/caps/gateway):
  - **workspace isolation (mandatory)** — a `series.find` query variable resolves the ws-B viewer's series,
    never ws-A's (token-scoped resolver, rule 6);
  - **capability deny (mandatory)** — a viewer lacking `mcp:series.find:call` gets options denied opaquely,
    the bar intact (rule 5).
- **Additivity** — the resolved `VarScope` shape is unchanged; a pre-advanced record's fields are all
  optional/serde-default (no `VARS_LIB_V` bump).

## Follow-ups (from the scope, not built)

- Chained resolution proven live is left to the unit layer (`orderVariables` + the resolvedArgs re-key);
  a full render-based chained gateway case is a natural next add.
- `adhoc` filters remain a named follow-up; `groupby` out of scope.
- Remaining format hints (`lucene`, datasource-language ones) deferred.
- Import/export mapper (Grafana `VariableModel` → these fields) lives in the import-export scope.
