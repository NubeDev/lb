# Session — Rhai flow node gets the shared code editor

**Ask:** In the flows UI, a `rhai` node's `source` should be edited in the real code
editor (the one the rules workbench uses), not a one-line text input.

## What shipped

- **Descriptor hint (opaque data, not a UI branch).** `crates/flows/src/builtins/core.rs`
  — the `rhai` node's `source` field now declares `"format": "rhai"` (plus a
  description). `format` is a standard JSON-Schema annotation; the config-schema
  validator ignores it, so values still validate exactly as before.
- **Reusable language resolver.** `ui/src/components/codeeditor/codeLanguage.ts` —
  `codeLanguageExtension(format)` / `isCodeFormat(format)` map an opaque `format`
  string (`rhai` → `lang-javascript`, `sql` → `lang-sql`) to a CodeMirror language
  extension. Lives next to `CodeEditor` so every code surface resolves a language
  the same way; exported from the `codeeditor` barrel.
- **Generic form rendering.** `ui/src/features/flows/SchemaForm.tsx` — a `string`
  field whose schema has a code `format` renders the shared `CodeEditor` instead of
  `<Input>`. The form never learns "rhai is a node type" — it only reads the schema
  hint, so any built-in **or extension** descriptor can opt a field into the editor
  (Decision 3 "no hardcoded UI"; rule 10 "core knows no extension").

## Why this seam (rejected alternative)

Rejected: special-casing `node.type === "rhai"` in `NodeConfigPanel`/`SchemaForm`.
That would branch core UI on a node id — a rule-10 leak, and it wouldn't extend to
a SQL field or a third-party extension's code field. Driving it off an opaque
schema `format` keeps the editor generic and reusable.

## Tests (green)

- `ui`: `pnpm test SchemaForm` — 9/9, including the new case asserting a
  `format: rhai` string field renders a `.cm-editor` (not an `<input>`).
- `rust`: `cargo test -p lb-flows` — 81/81 (descriptor still compiles + validates).
- `tsc --noEmit` clean.

## Open

- The executed-node **lock** (`disabled`) does not yet propagate to the code editor
  — `CodeEditor` has no read-only prop. A follow-up can thread
  `EditorState.readOnly.of(true)` through when `disabled`. Low risk: the lock's
  primary gate (no Save/Patch button) still holds.
