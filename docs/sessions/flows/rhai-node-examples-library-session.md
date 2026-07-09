# Session — Rhai flow node examples library

**Ask:** Add an examples library for the rhai flow node (like the rules examples) —
~20 examples in categories, each with the code visible in a dropdown and a copy button.

## What shipped

- **Catalog (data).** `ui/src/features/flows/examples/rhaiExamples.ts` — 21 examples in
  5 categories (Basics, Shaping payloads, Conditions & routing, Arrays & aggregation,
  Findings & side-effects). Bodies are grounded in the flow rhai convention: the message
  envelope's fields (`payload`, `topic`, …) are top-level variables; a node returns a
  value or emits (`emit`/`alert`/`log`). `RHAI_EXAMPLE_CATEGORIES` + a flattened
  `RHAI_EXAMPLES`.
- **Library UI.** `ui/src/features/flows/examples/RhaiExampleLibrary.tsx` — categorized
  list; each row is a **collapsible dropdown** (title + summary; click to reveal the
  code in a `<pre>`), with a **Copy** button (clipboard + a brief "Copied" confirm) and
  a **Use in editor** button that loads the body into the source buffer.
- **Wired below the editor.** `ui/src/features/flows/SchemaForm.tsx` — a code field
  (from the earlier `format` seam) may register a below-editor helper via
  `CODE_FIELD_HELPERS`; `rhai → RhaiExampleLibrary`. So the library renders **under** the
  rhai `source` editor in the node Config panel. Hidden when the field is `disabled`
  (the executed-node lock).

## Why this seam

Keyed on the opaque `format: "rhai"` schema hint (not `node.type === "rhai"`) — the same
data-driven seam the code-editor swap uses, so no core-UI branch on a node id (rule 10).
`CODE_FIELD_HELPERS` is a plain map: a future language (SQL) can register its own helper
without touching the field-render logic. Placement chosen with the user: **below the
editor** (closest to the code) over a separate dock tab.

Reuses the rules-examples pattern (a static catalog + a click-to-load list) rather than
inventing a new one; the flow version adds the per-row dropdown + copy the ask called for.

## Tests (green)

- `pnpm test RhaiExampleLibrary` — 4/4 (catalog ≥20 & categorized & unique ids; code
  hidden until expand; Use loads the body; Copy writes to clipboard via a stubbed
  `navigator.clipboard`).
- `pnpm test SchemaForm` — 10/10, incl. the library rendering below a `format: rhai`
  field AND being hidden when `disabled`.
- `tsc --noEmit` clean.
- Pre-existing `flows/debug/DebugValueView` (2) still fail — in-flight branch work, not
  this change (see [[preexisting-failing-tests]]).

## Notes

- UI-only change (vite hot-reloads); no node rebuild needed — unlike the descriptor
  `format` hint, which did (see the rhai-node-code-editor session +
  [[flows-descriptor-served-not-hardcoded]]).
