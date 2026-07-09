// Maps an opaque code `format` string (as declared on a JSON-Schema string field, e.g. a flow node's
// descriptor) to the matching CodeMirror language extension. Kept next to `CodeEditor` so every
// surface that renders code — the rules workbench, the flows node-config form, a future extension
// field — resolves a language the SAME way instead of re-listing `lang-*` imports. This is pure
// data-driven mapping: a caller passes a format string it read from a schema; nothing here knows what
// a "rhai node" is, so any built-in or extension descriptor opts a field in by declaring the format
// (Decision 3 — no hardcoded UI). One responsibility per file (FILE-LAYOUT).

import { javascript } from "@codemirror/lang-javascript";
import { sql } from "@codemirror/lang-sql";
import type { Extension } from "@codemirror/state";

/** The code `format`s a string field may declare to render in the CodeMirror surface. */
const CODE_LANGUAGES: Record<string, () => Extension> = {
  // Rhai is JS-like, so `lang-javascript` highlighting is good enough (matches RuleEditor — the
  // shipped dep, no Monaco).
  rhai: javascript,
  sql,
};

/** Resolve a schema `format` to its CodeMirror language extension, or `undefined` if the format is
 *  not a code language (the caller then falls back to a plain input). */
export function codeLanguageExtension(format: unknown): Extension | undefined {
  if (typeof format !== "string") return undefined;
  const lang = CODE_LANGUAGES[format];
  return lang ? lang() : undefined;
}

/** True when a schema `format` names a code language (a string field should render as an editor). */
export function isCodeFormat(format: unknown): boolean {
  return typeof format === "string" && format in CODE_LANGUAGES;
}
