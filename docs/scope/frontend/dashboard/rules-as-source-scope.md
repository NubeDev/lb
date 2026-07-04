# Frontend scope — a saved rule is a picker source (Data Studio: query-with-a-rule → chart)

Status: **scope (the ask)** — 2026-07-04. Parent: the reusable source picker
([`source-picker-package-scope.md`](source-picker-package-scope.md)) and the rules cage
([`../../rules/rules-engine-scope.md`](../../rules/rules-engine-scope.md),
[`../../rules/data-stdlib-scope.md`](../../rules/data-stdlib-scope.md)).
Promotes to `public/frontend/` once shipped.

## The ask

In Data Studio, let a user **build a chart/panel whose data comes from a rule** — a rhai script that
fetches from the gated sources (`source("cooler.temp")`, a datasource, `query:<id>`), reshapes and
computes over the rows (the data-stdlib: `time`, `stats`, polars `Frame`), and **returns records**.
The rule does the *query + compute*; the panel does the *draw*. This is the "rules as an example for
Data Studio" idea: a rule is the most general query — anything you can express in the cage becomes a
chart.

Concretely: a **saved rule should be one more thing the source picker offers**, in a **Rules** group,
alongside Series / Live / SQL / Extension / Flows.

## Why this is (almost) already built

The source picker's output vocabulary is a `SourceSelection` whose `.source` is `{ tool, args }` —
**any granted MCP tool call**, re-gated at the host per call ([`types.ts`](../../../../packages/source-picker/src/types.ts)).
A saved rule is **already an MCP verb**: `rules.run { rule_id, params } → { output, findings, log, ms }`
([rules-engine-scope.md §"The verbs"](../../rules/rules-engine-scope.md)). And a `data = true` panel
(echarts-panel is the reference) already receives shell-resolved `ctx.data` frames from a cell's
`sources[]` ([[ext-widget-frames-in-contract]]). So the whole pipeline exists:

```
rule author (rhai + data-stdlib)          source-picker              dashboard cell → panel
──────────────────────────────            ─────────────              ──────────────────────
source("cooler.temp").last("24h")   ⇒     Rules group entry     ⇒    sources[] = [{tool:"rules.run",
frame(rows).group_by("hour").mean()       { tool:"rules.run",         args:{rule_id}}]  →  ctx.data
return f.records()                          args:{rule_id} }           →  echarts draws it
```

The **only** missing seam is that the picker doesn't yet enumerate saved rules. No new host chokepoint,
no new panel contract, no rule-runtime change — the rule is reached through the exact generic gated
seam every other source uses (`mcp:rules.run:call`, re-checked per call), so CLAUDE §10 holds
(the core never learns "rules is special" — it's one more opaque tool id the picker offers).

## What exists today (the mirror to copy)

The **Flows** group is the exact precedent: `listFlows`/`getFlow`/`listFlowNodes` loaders →
`flowsEntries()` → a `flows` group whose OUTPUT entries resolve to a read `{tool,args}` source
([`sourcePicker.ts` `flowsEntries`](../../../../packages/source-picker/src/sourcePicker.ts)). Rules are
strictly simpler than flows (no node/port walk): one loader, one entry per rule.

## Intent / approach

**Add a `rules` group by mirroring the flows plumbing, one layer at a time.** No behaviour is novel; it
is the same DI seam the package already documents ("host injects a `SourceLoaders`").

1. **Type** — add `listRules?: () => Promise<RuleSummary[]>` to `SourceLoaders`, and a `RuleSummary`
   row shape (`{ id: string; name: string }`, mirroring `FlowSummary` — the subset of `rules.list` the
   picker needs). Optional like every loader: a host without the `mcp:rules.list:call` grant simply
   omits it and the group is absent (honest, capability-scoped — the deny-tolerant rule, CLAUDE §9).

2. **Model** — add `rulesEntries(rules: RuleSummary[]): SourceEntry[]`. Each rule ⇒ one read entry:
   ```ts
   { id: `rule:${r.id}`, group: "rules", label: r.name || r.id,
     source: { tool: "rules.run", args: { rule_id: r.id } }, writes: false }
   ```
   Add `"rules"` to `SourceEntry.group`'s union. Fold into `buildSourceEntries` (new `rules?` input)
   next to flows.

3. **Loader** — `loadSourcePicker` calls `listRules` in its deny-tolerant `Promise.all` (empty on
   reject), feeds `rulesEntries`. `useSourcePicker` needs no change (it delegates to `loadSourcePicker`).

4. **UI** — add `{ group: "rules", label: "Rules" }` to `READ_SOURCE_GROUPS` (and, since a rule is a
   read data source, to the read list only — not `BUILDER_SOURCE_GROUPS`'s control intent). The
   grouped `<select>` renders it with zero component change.

5. **Params (host-side, out of package scope).** A rule with required `params` needs values supplied.
   That is host-specific target shaping — exactly like the flow node→port sub-picker the README says a
   host composes *around* this package. v1 of the package offers the rule with no `params`
   (`args: { rule_id }`); a rule that needs params either defaults them or the host wraps the picker
   with a small params form. Called out as an **open question**, not built into the package.

**Return shape is a convention, not a code gate.** A chart wants a **frame-shaped** return (array of
row maps, e.g. `f.records()`), not `{kind:"findings"}`. The picker surfaces every saved rule; a rule
whose output isn't records renders as an empty/misshaped panel — an honest failure, the same as a
denied loader. We do **not** try to statically classify "is this rule a data rule" in v1 (the cage
can't be introspected for output shape without running it). Open question: tag data-rules at save time.

## Non-goals

- **No new rule runtime, no new host chokepoint.** The rule runs through `rules.run` unchanged; the
  panel consumes `ctx.data` unchanged. This scope is purely "the picker enumerates saved rules."
- **No rule authoring in Data Studio.** Authoring a rule stays the rules Playground's job; Data Studio
  *consumes* a saved rule by id. (A "new rule" affordance from the panel editor is additive, later.)
- **No params UI in the package.** See intent §5 — host composes it, per the source-picker doctrine.
- **No branch on the `rules` id in any core crate or the core shell.** `rules.run` is an opaque tool id
  to the picker and the cell; the host gates it generically (`mcp:rules.run:call`). CLAUDE §10.

## First customer

The lb dashboard shell — its `SourceLoaders` gains a `listRules` backed by the `rules.list` client, so
a Data Studio panel's Query tab shows a **Rules** section. (`thecrew` and any other picker consumer get
it for free the moment they inject `listRules`.)

## Testing plan

Per [`../../testing/testing-scope.md`](../../testing/testing-scope.md):

- **Model unit** (`sourcePicker.test.ts`): `rulesEntries([{id:"r1",name:"Hourly mean"}])` ⇒ one `rules`
  entry with `source.tool === "rules.run"`, `args.rule_id === "r1"`; `buildSourceEntries({rules})`
  includes it; `selectionOf` folds it to a `SourceSelection` with that source.
- **Loader deny-tolerance** (`useSourcePicker.test.tsx` / `loadSourcePicker`): a `listRules` that
  **rejects** ⇒ the Rules group is empty, every other group still loads (capability-scoped offer, §9).
  An absent `listRules` ⇒ no Rules group, no throw.
- **UI** (`SourcePicker.test.tsx`): given a rules entry, the `<select>` renders a "Rules" `<optgroup>`
  with the rule's label; picking it fires `onSelect` with the `rules.run` source.
- **Real end-to-end (gateway)**: the mandatory no-mock path — seed a saved rule that returns records
  into the real store, drive `rules.list`/`rules.run` over a spawned gateway, assert the picker offers
  it and the source resolves to real rows. (Extends the package's gateway coverage where it exists;
  otherwise noted as the shell-side integration test on adoption.)
- **Capability-deny + workspace-isolation** (mandatory): a workspace without `mcp:rules.list:call`
  gets no Rules group; a rule saved in `acme` is not offered to `beta` (the ws wall — the loader is
  ws-keyed and the host gates `rules.list` workspace-first).

## Open questions

1. **Params.** Should the package grow an optional `params` schema on `RuleSummary` (from `rules.list`)
   so a host can render a generated form, or does every host shape params itself? Lean: host-shaped in
   v1 (matches flows), revisit if two hosts duplicate a form.
2. **Data-rule tagging.** Mark a rule as "returns records" at save time (a `rules.save` flag) so the
   picker can offer *only* chartable rules? Or surface all and let a bad shape fail honestly? Lean:
   surface all in v1; add an optional filter if noise is real.
3. **Live rules.** A rule is a single bounded run (`rules.run`). A *chain* streams (rules-engine §"Live
   feed"). Out of scope here; a live-rule source would mirror the `live`/`series.watch` entry later.
