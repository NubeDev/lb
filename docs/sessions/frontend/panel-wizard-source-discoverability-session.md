# Session — new-panel wizard: make "bind a panel to a saved RULE" discoverable

Date: 2026-07-09. Branch: `insights-updates`. Scope:
[`../../scope/frontend/dashboard/panel-wizard-source-discoverability-scope.md`](../../scope/frontend/dashboard/panel-wizard-source-discoverability-scope.md).

## The ask

A user who wanted to bind a new panel to a saved **rule** couldn't tell how from wizard step-1. The
three Source cards (Insights / Workspace source / Datasource) named "rule" only in the Workspace-source
subtitle; the Rules group sat seventh in a flattened combobox behind that card. Users clicked
*Datasource* instead and got stuck in the SQL workbench. The picker + render halves already shipped
(`rules-as-source` / `rules-for-widgets`) — this was a pure discoverability/labelling gap.

## Decision

Kept the three intent-bucket cards (rejected a 5th "Rule" card — it would fracture one generic picker
seam into per-kind cards and break the three-bucket scan; CLAUDE §10). Fixed the scent instead. The
rejected alternative + why is recorded in the scope.

## Changes (labelling/order only — no new track, no flow-logic branch)

- [`ui/src/features/panel-builder/wizard/SourceStep.tsx`](../../../ui/src/features/panel-builder/wizard/SourceStep.tsx):
  - Workspace-source card subtitle → "Bind to a saved rule, series, or saved query already in this
    workspace." (front-loads the word the user hunts).
  - New `WORKSPACE_SOURCE_GROUPS` — derived from the canonical `READ_SOURCE_GROUPS` (one vocabulary),
    `flows` dropped, **`rules` hoisted first** — passed to the workspace `SourceCombobox`.
  - An empty-Rules line (`aria-label="wizard no rules"`) shown when the loaded entries carry no `rules`
    group entry (covers both "no rules yet" and the deny-tolerant "no `mcp:rules.list:call`" case — the
    combobox renders no heading for an empty group, so the affordance lives beside it in the wizard).
- No change to the shared `@nube/source-picker` package: `SourceCombobox` already accepts a `groups`
  order prop; only the wizard's chosen order changed.

## Tests (real gateway, rule 9 — no fakes)

New `ui/src/features/panel-builder/wizard/sourceStepRuleDiscoverability.gateway.test.tsx` — 6 tests
against a spawned `test_gateway` node + a real `rules.save`d rule:

1. ≤2 clicks from step-1 to the rule; "Rules" is the **first** group heading in the combobox.
2. Picking the rule adopts `{tool:"rules.run", args:{rule_id, route:false}}` (readout asserts it).
3. Card-scent regression: the Workspace-source card's accessible text contains "rule".
4. Empty-Rules honest line shows when no rule is seeded.
5. **Capability-deny**: without `mcp:rules.list:call`, the Rules group is empty + the honest line shows,
   the seeded rule is never offered, no crash.
6. **Workspace isolation**: a rule saved in workspace A is not offered in the wizard opened in B.

### Green output

```
✓ src/features/panel-builder/wizard/sourceStepRuleDiscoverability.gateway.test.tsx (6 tests) 436ms
  Test Files  1 passed (1)   Tests  6 passed (6)
```

Full wizard-source suite (regression — the reordered group list must not break siblings that drive
`source track workspace` → `wizard source`):

```
Test Files  8 passed (8)   Tests  27 passed (27)
```

(CodeMirror `getClientRects`/`coordsAt` stderr lines are a pre-existing jsdom measurement warning in
the template-editor tests, not a failure.)

## Docs

- Promoted the shipped behaviour to
  [`doc-site/content/public/dashboard/dashboard.md`](../../../doc-site/content/public/dashboard/dashboard.md)
  → "New-panel wizard — binding a panel to a saved rule".
- Flipped the scope + index README entries to SHIPPED (2026-07-09).

Nothing broke (no `docs/debugging/` entry needed). Not committed — branch `insights-updates`, per the
user's instruction to manage git.
