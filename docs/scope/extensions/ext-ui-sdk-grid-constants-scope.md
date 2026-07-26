# Extensions scope — export the dashboard grid constants from `@nube/ext-ui-sdk`

Status: **stub scope (the ask)**. Consumer-driven, doc-only, deliberately small — this records a
debt an extension has already filed against itself, not a design. Written 2026-07-26.

First consumer:
`NubeIO/rubix-ai-extensions → extensions/modbus/docs/scope/template-dashboard-visual-builder-scope.md`
(§3.2 / D2 — the mirror; D10 — this file). Read that scope for the product motivation; it ships
correct **without** this and carries the sharp edge meanwhile.

## The gap

An extension that **authors** host dashboard cells has to place them on the host's grid. The
geometry that decides what `{x,y,w,h}` means — the column count, the row height, the inter-item
margin — is a host presentation fact, and **no published contract exposes it**.

`modbus` has just shipped a visual builder for its managed template board. Its canvas emits
`cells[]` in the host's versioned panel schema and lays them out with the host's own
`react-grid-layout@1.5.3`, precisely so that a 6-wide cell in the authoring canvas is a 6-wide cell
in the viewer. To do that it needs the numbers, so it carries a hand-copied mirror:

- `rubix-ai-extensions → extensions/modbus/ui/src/features/templates/dashboard/canvas/grid.ts` —
  `GRID_COLS = 12`, `GRID_ROW_H = 56`, `GRID_MARGIN = 10`, plus a local `boardPixelHeight`.
- Upstream truth: `rubix-ai → ui/src/features/dashboard/gridGeometry.ts` — the same three constants
  plus `boardPixelHeight` and `fitScale`.

**Why this is a defect and not a nit.** If the host's column count or row height ever changes, the
extension's canvas keeps placing cells on the old grid, and the failure is **silent and geometric**:
a board that looks right where it was authored and reflows where it is viewed. That is exactly the
failure the builder's own D9 rejected a hand-rolled grid to avoid — it re-enters through the
constants instead of the layout engine.

**Why the extension cannot fix this itself.** The options available to an out-of-tree extension are
(a) mirror the constants in one owner file — what `modbus` did, and filed as a debt — or (b)
hardcode them across several components, which is strictly worse. There is no third option that does
not involve a published export. The extension is already doing the best available thing; the fix has
to be in the SDK.

**Why `@nube/ext-ui-sdk` is the right layer.** It is already the published seam for the page and
widget contracts an extension consumes, and per `rubix-ai-extensions → CLAUDE.md` a contract change
is a versioned SDK release, never a sibling edit. An extension reading the grid geometry from the
SDK it already pins is the same shape as everything else it reads from there.

## Goals

- **Export `GRID_COLS`, `GRID_ROW_H`, `GRID_MARGIN`** from `@nube/ext-ui-sdk`, with the same values
  the shell lays boards out with, so an extension authoring cells reads them instead of copying
  them.
- **Include `boardPixelHeight`** — the natural companion. Every consumer that needs a canvas height
  derives it from the same three numbers, and a second derivation is a second place to drift.
- **Keep it additive.** New exports on the existing package; no change to the mount signature, the
  page contract, the widget contract, or any host verb.

## Non-goals

- **`fitScale` does not belong in the SDK.** It encodes edit-mode-bay zoom behaviour (never scales
  up, legibility floor) — host UX, not a layout contract. An extension that needs a zoom affordance
  should decide its own, not inherit the shell's.
- **No grid *component* in the SDK.** This publishes numbers, not a renderer. An extension brings
  its own `react-grid-layout` (each extension owns its deps) — the constants are what make the two
  layouts mean the same thing.
- **No new capability, no new verb, no host change.** Grid geometry is not gated; it is a published
  presentation constant.

## How it fits the core

- **Rule 10 holds.** This grants no extension anything special: it publishes one presentation fact
  that every extension gets equally, through the SDK every extension already pins. No core branch on
  an extension id, nothing reserved for a named ext, no capability-grammar change. Swapping `modbus`
  for an equivalent extension exercises the identical export.
- **Capabilities / tenancy / MCP surface:** unchanged. Nothing here is reachable state.
- **SDK impact — flagged.** Additive TypeScript exports, so a **minor `ui-v*` tag**. Consumers move
  at their own pace: a consumer keeps its mirror until it can pin the new tag, then deletes the
  mirror file and imports from the SDK. Nothing breaks for a consumer that never bumps.
- **Where the values come from** is an implementation question for the SDK release, not this ask:
  they must be the same numbers the shell renders with, and the release must not create a *third*
  copy that can drift from `gridGeometry.ts`.

## The interim state (stated honestly)

Until this lands, `modbus`'s mirror is a **known, filed debt** with two mitigations:

1. **One owner file** — `canvas/grid.ts` names the upstream truth and this scope in its header, and
   nothing else in that extension hardcodes a column count, row height, or margin.
2. **The geometry-parity check** in the builder scope's §10.2 — generate a board, compare the
   extension canvas against the host viewer for the same board bound to a stamped device; every cell
   must occupy the same column span and row offset.

Filing it is the point. An unfiled mirror is a future rediscovery, at the moment the numbers change.

## Testing plan

Small, and mostly downstream (rule 9 — real node, no fixtures of host behaviour):

- **SDK:** the exports exist, carry the shell's values, and `boardPixelHeight` matches the shell's
  result for the same row count.
- **Downstream:** the first consumer deletes its mirror, pins the new tag, and re-runs the §10.2
  geometry-parity check against a real board in a real viewer. That check is the real assertion —
  the export is only worth anything if parity still holds after the mirror is gone.

## Open questions

- Does the SDK export bare constants, or a single frozen `grid` object? Bare constants match both
  existing files and keep tree-shaking trivial; decide at release.
- Should the shell's `gridGeometry.ts` become a *consumer* of the SDK export, so there is exactly one
  definition rather than two that agree? Cleaner, but a bigger change than this ask needs — noted,
  not required.

## Related

- `NubeIO/rubix-ai-extensions → extensions/modbus/docs/scope/template-dashboard-visual-builder-scope.md`
  — **the originating consumer**: §3.2 / D2 (the mirror and why it drifts), §10.2 (the parity check),
  §11 (the risk row), D10 (this file, the only lb-side deliverable of that session).
- `ext-out-of-tree-scope.md` — the published-SDK split that makes copying the only alternative.
- `ext-widget-panel-options-scope.md` — the precedent for an additive `@nube/ext-ui-sdk` type/contract
  release driven by a downstream authoring surface.
- `ext-managed-dashboards-scope.md` — the other platform ask from the same consumer's board.
