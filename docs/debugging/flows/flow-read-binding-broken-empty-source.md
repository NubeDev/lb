# flow read cell shows "binding broken — re-pick" despite a working backend read

**Area:** flows (dashboard binding) · **Status:** resolved · **Date:** 2026-07-01

## Symptom

A dashboard cell bound to a flow node's output (`jsonview` / `stat` over `flows.node_state`) rendered
**"binding broken — re-pick"** in the grid, even though:
- the picker's preview showed the value correctly,
- `GET /flows/{id}/node_state` returned the real value (`counter-2 → {payload, ts}`),
- earlier gateway tests that hand-built the cell and rendered it through `WidgetView`/`WidgetHost`
  **passed** (so "it's fixed" was reported repeatedly — but the live grid still broke).

## Root cause

The passing tests hand-built a cell with **no** `cell.source` key. The REAL cell — saved by the
PanelEditor and round-tripped through the gateway — carries an **empty v2 placeholder** alongside the v3
`sources[]`:

```json
"source": { "tool": "", "args": null },
"sources": [{ "tool": "flows.node_state", "args": { "id": …, "__flowNode": …, "__flowPath": ["ts"] } }]
```

`views/WidgetView.tsx` resolved the primary source as:

```ts
const primarySource = cell.source ?? (primaryTarget ? { tool, args } : undefined);
```

`{tool:"", args:null}` is a **present** object, so `??` returned it instead of falling through to the
real `sources[0]` flow target. The reader then ran `flowBindingOfSource({tool:""})` → `null` → no
`flowId` → `denied:true` → "binding broken." The backend was never the problem; the empty placeholder
shadowed the real target on the render path.

## Fix

Use the v2 `source` only when it carries a real tool; otherwise fall back to the v3 primary target
(`views/WidgetView.tsx`):

```ts
const primarySource = cell.source?.tool
  ? cell.source
  : primaryTarget ? { tool: primaryTarget.tool, args: primaryTarget.args } : undefined;
```

## Regression

`views/flowTsDisplay.gateway.test.tsx` renders the EXACT saved shape — including
`source: {tool:"", args:null}` beside the real `sources[]` — through `WidgetHost` (the grid's real render
path) against a real gateway, and asserts the value renders (never "binding broken"). The earlier tests
missed it precisely because they omitted the placeholder; this one reproduces the round-tripped cell.

## Lesson

Test the **exact shape the store round-trips**, through the **real render host** — not a hand-built cell
through the component. An empty-but-present field is truthy; `??` is the wrong operator when a field can
be present-but-empty. Prefer `x?.tool ? x : fallback` over `x ?? fallback` for "has a real value" checks.
