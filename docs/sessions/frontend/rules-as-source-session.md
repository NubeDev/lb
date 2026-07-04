# Frontend — a saved rule is a picker source (Rules group in `@nube/source-picker`) (session)

- Date: 2026-07-04
- Scope: ../../scope/frontend/dashboard/rules-as-source-scope.md
- Stage: post-S8; a package-level add on the shipped source picker, no stage gate of its own.
- Status: done — package layer + shell adoption (`listRules` on the shell `SourceLoaders`) + real-gateway ws-isolation/deny test all shipped.

## Goal

Let Data Studio build a chart/panel whose data comes from a **rule** — a rhai script that fetches from
the gated sources, computes over the rows (the data-stdlib: time/stats/polars `Frame`), and returns
records the panel draws. Concretely: make a saved rule one more thing the source picker offers, in a
**Rules** group, resolving to a `rules.run {rule_id}` read source — reusing the exact generic gated
seam every other source uses (no new host chokepoint, no panel-contract change, no rule-runtime change).

## What changed

All in `packages/source-picker/src/` — the Flows group was the mirror to copy (a rule is strictly
simpler: one loader, one entry per rule, no node/port walk).

- `types.ts` — new `RuleSummary` row shape (`{id,name}`, mirrors `FlowSummary`); new optional
  `SourceLoaders.listRules?()` seam (deny-tolerant like every loader).
- `sourcePicker.ts` — new `rulesEntries()` (rule ⇒ `{tool:"rules.run",args:{rule_id}}` read source);
  added `"rules"` to `SourceEntry.group`; `rules?` input on `SourceInputs`, folded into
  `buildSourceEntries` next to flows.
- `loadSourcePicker.ts` — `listRules` added to the deny-tolerant `Promise.all` (empty on reject), fed
  into `buildSourceEntries`. `useSourcePicker` unchanged (delegates here).
- `SourcePicker.tsx` — `{group:"rules",label:"Rules"}` appended to `READ_SOURCE_GROUPS` (read intent
  only, not the builder/control list). No component logic change.
- `index.ts` — export `rulesEntries` + the `RuleSummary` type.
- `README.md` — documented the `listRules` loader in the Wiring block.

## How it fits the core

CLAUDE §10 holds: `rules.run` is an opaque tool id to the picker and the cell; the host gates it
generically (`mcp:rules.run:call`), re-checked per call. No core crate or core shell branches on the
`rules` id. CLAUDE §9 holds: a host without `mcp:rules.list:call` omits `listRules` and the group is
absent (honest, capability-scoped offer) — no fabricated catalogue.

The return-shape (records vs `{kind:"findings"}`) is a rule-author convention, not a code gate: the
picker surfaces every saved rule; a non-record rule renders as an empty/misshaped panel, an honest
failure like a denied loader. Not statically classified in v1 (the cage can't be introspected without
running it) — see scope open question #2.

## Tests (green)

`cd packages/source-picker && pnpm test` → **21 passed** (was 15). `pnpm exec tsc --noEmit` clean.

- `sourcePicker.test.ts` — `rulesEntries` maps to a `rules.run` source keyed by `rule_id`; id-fallback
  label; folded under the `rules` group by `buildSourceEntries`.
- `useSourcePicker.test.tsx` — the full loader set now yields a `rules` group; a **rejected** `listRules`
  yields no Rules group while every other group still loads (the mandatory capability-deny path, at the
  loader seam); an omitted loader → absent group.
- `SourcePicker.test.tsx` — the `<select>` renders a "Rules" `<optgroup>`; picking a rule fires
  `onSelect` with the `rules.run` source.

## Shell adoption (`ui/`)

- `ui/src/features/dashboard/builder/useSourcePicker.ts` — the ONE shell adapter now injects
  `listRules: () => listRules()` (from `@/lib/rules/rules.api`, which already existed returning
  `SavedRule[]`, a structural superset of the package's `RuleSummary`). The Data Studio Query tab's
  `<select>` gains a **Rules** section with zero further change (the package renders the group). Had to
  `pnpm build` the package first — the shell resolves `@nube/source-picker` to `dist`, not `src`.

**Workspace-isolation / real-gateway (now shipped, not a follow-up):**
`ui/src/features/dashboard/builder/rulesSource.gateway.test.tsx` — drives the package's real
`loadSourcePicker` through the shell's `listRules` client against a **real in-process gateway** (no
mocks). 3 tests, green in 24s (includes the Rust build):
- a saved rule surfaces as a `rules.run {rule_id}` read source;
- **the hard wall:** a rule saved in one ws is NOT offered in another;
- **capability-deny:** a ws without `mcp:rules.list:call` gets an EMPTY Rules group (deny → empty, no
  crash, no fabricated entry — §9).
Adjacent `flowsPicker`/`widgetBuilder` unit tests re-run green (no regression).

## Params form (scope open Q1 — now shipped)

A rule with declared `params` now gets a form in the Data Studio Query tab; the values ride in the
`rules.run` target's `args.params` and reach `param("<name>")` in the cage.

- **Package** (`packages/source-picker/`): `RuleParam {name,label?}` added; `RuleSummary.params?`
  carries them; `rulesEntries` puts them on `SourceEntry.params` (defaulting `[]`). Exported
  `RuleParam`. Rebuilt `dist` (the shell resolves the built package).
- **Shell adapter**: no change — `SavedRule.params` (already returned by `listRules`) structurally
  satisfies `RuleSummary.params`, so params flow through automatically.
- **`ui/.../panel-builder/tabs/RuleParamsSection.tsx`** (new): one input per param, writes
  `args.params`; an empty field is OMITTED (rule sees an absent param, not `""`); no form when a rule
  declares none.
- **`ui/.../panel-builder/tabs/QueryTab.tsx`**: renders `RuleParamsSection` when the primary target's
  tool is `rules.run` and the resolved entry is a rule.
- **`ui/.../dashboard/builder/sourcePicker.ts`** (`seedEntryId`): now disambiguates rules by `rule_id`
  (every rule shares `tool === "rules.run"`, so tool-only match collapsed them all to the first rule —
  the same way `series` disambiguates the shared `series.read`). This was a latent bug the params form
  surfaced (the wrong rule's params would show).

### Tests (green)

- `QueryTab.test.tsx` — 9 tests (was 5): params inputs render per declared param; an edit writes
  `args.params`; clearing omits the key; a no-param rule shows no form.
- `rulesSource.gateway.test.tsx` — 5 tests (was 3): entry carries `params`; **a param-driven rule runs
  end-to-end against the real node** — `runRule({params:{site}})` → `param("site")` → the value comes
  back in the output (the exact `args.params` shape the form builds). Green in ~37s (incl. Rust build).
- `flowsPicker`/`widgetBuilder` re-run green (no regression).

## Typed params + authoring loop (scope open Q1 refinement — now shipped)

Params are no longer string-only, and a user can DECLARE them in the UI (not just via a raw `saveRule`).

**Backend** (`rust/crates/rules/src/runtime.rs`): `RuleParam` gains `kind: ParamKind`
(`text|number|date|enum`, serde `lowercase`), `required: bool`, `options: Vec<String>` — all
`#[serde(default)]`, so a legacy `{name,label}` record deserializes unchanged (**no migration**).
`ParamKind` exported from `lb_rules`. `rules.save`/`rules.get` need no change (they `from_value` the
list). Unit tests in-file: legacy-defaults + typed round-trip (kind serializes lowercase).

**Wire + package**: `ParamKind` + the new fields mirrored on the shell `RuleParam`
(`ui/src/lib/rules/rules.types.ts`) and the package `RuleParam` (`packages/source-picker`), rebuilt
`dist`. Both `#[serde(default)]`/optional so legacy shapes are unaffected.

**Consumer form** (`RuleParamsSection.tsx`): renders per `kind` — `<input type=number|date|text>` /
`<select>` for enum; a NUMBER coerces to a JS number into `args.params` (so the cage sees a rhai
number); an empty non-required field is omitted; a required-empty field is `aria-invalid`.

**Authoring UI**: new `ui/src/features/rules/panel/ParamDeclEditor.tsx` (add/remove/edit rows: name,
label, kind, required, enum options) mounted as a new **Params** tab in `AuthoringPanel`. `useRules`
now co-owns a `params` buffer with the body: seeded on `open`, reset on `newRule`/`loadExample`,
threaded through `create`/`save`/`rename`, and folded into the `dirty` check (a param-only edit enables
Save). `RulesView` wires `params`/`setParams` to the panel.

**A latent-adjacent fix**: `seedEntryId` now disambiguates rules by `rule_id` (all rules share
`tool === "rules.run"`) — without it the wrong rule's params would show.

### Tests (green)

- `lb-rules` lib: 2 serde tests (legacy defaults, typed round-trip).
- `QueryTab.test.tsx` — 11 (was 9): +number coercion to JSON number, +enum renders a `<select>`.
- `ParamDeclEditor.test.tsx` (new) — 5: add/edit/remove, enum reveals+parses options, required toggle.
- `rulesSource.gateway.test.tsx` — 7 (was 5): +typed param **save → get → picker entry** round-trip
  (real node), +**number type preserved into the cage** (`param("n")+1 == 25`, not `"241"`). Also
  updated the pre-typed-params assertion to the node's full serialized shape (kind/required/options).
- Package — 24: +typed param carried onto the entry.

### Not mine (pre-existing, concurrent session)

`RulesView.gateway.test.tsx` fails 11/11 with `useTheme must be used within ThemeProvider` (from
`lib/motion/Reveal` — the in-flight theme/motion refactor). **Verified it fails identically with all my
changes stashed** — not a regression from this work. The gateway itself now compiles (an earlier
mid-edit `pin_dashboards` break in `role/gateway/routes` from the same concurrent session has settled).

## Follow-ups

- **Required-empty enforcement**: the form flags a required-but-empty param but doesn't yet block the
  panel run — wire it to the run/preview gate.: a rule with required `params` needs a host-composed form around the
  picker (like the flow node→port sub-picker). v1 offers `args:{rule_id}` only.
- **Data-rule tagging** (scope open Q2) and **live rules** (open Q3) remain open.
