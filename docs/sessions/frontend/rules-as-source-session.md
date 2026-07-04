# Frontend — a saved rule is a picker source (Rules group in `@nube/source-picker`) (session)

- Date: 2026-07-04
- Scope: ../../scope/frontend/dashboard/rules-as-source-scope.md
- Stage: post-S8; a package-level add on the shipped source picker, no stage gate of its own.
- Status: done (package layer); host adoption (`listRules` on the shell `SourceLoaders`) is the follow-up.

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

**Workspace-isolation / real-gateway:** the package is transport-agnostic (no node in-process), so the
ws-wall + real-store end-to-end assertion lands on **host adoption** — when the shell wires `listRules`
to its `rules.list` client, per the scope's testing plan (seed a records-returning rule in `acme`,
drive `rules.list`/`rules.run` over a spawned gateway, assert `beta` is not offered it). Noted as the
follow-up, not silently skipped.

## Follow-ups

- **Host adoption:** add `listRules` to the shell's `SourceLoaders` (backed by the `rules.list` client)
  so the Data Studio Query tab shows the Rules section; carry the real-gateway ws-isolation test there.
- **Params** (scope open Q1): a rule with required `params` needs a host-composed form around the
  picker (like the flow node→port sub-picker). v1 offers `args:{rule_id}` only.
- **Data-rule tagging** (scope open Q2) and **live rules** (open Q3) remain open.
