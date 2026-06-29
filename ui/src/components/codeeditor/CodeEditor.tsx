// A reusable controlled code editor with an imperative insert-at-cursor handle (rules-editor-ux scope).
// Wraps `@uiw/react-codemirror` and exposes `insertSnippet(text)` via a ref so any sibling panel (a
// function palette, a data explorer) can drop a snippet at the cursor — the click-to-insert primitive,
// extracted so a future editor surface reuses it rather than re-wiring CodeMirror. The caller supplies
// the language extension (Rhai → `lang-javascript`, SurrealQL → `lang-sql`), so this stays language-
// neutral. One component per file (FILE-LAYOUT).

import { forwardRef, useImperativeHandle } from "react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import type { Extension } from "@codemirror/state";

import { useEditorInsert } from "./useEditorInsert";

export interface CodeEditorHandle {
  /** Insert `text` at the cursor (replacing any selection). */
  insertSnippet: (text: string) => void;
}

interface CodeEditorProps {
  value: string;
  onChange: (value: string) => void;
  /** Language + theme extensions (e.g. `[javascript()]`). */
  extensions: Extension[];
  height?: string;
  ariaLabel?: string;
}

/** A controlled CodeMirror editor exposing an `insertSnippet` ref handle. */
export const CodeEditor = forwardRef<CodeEditorHandle, CodeEditorProps>(function CodeEditor(
  { value, onChange, extensions, height = "100%", ariaLabel = "code editor" },
  handleRef,
) {
  const { ref, insertSnippet } = useEditorInsert();
  useImperativeHandle(handleRef, () => ({ insertSnippet }), [insertSnippet]);

  return (
    <CodeMirror
      ref={ref as React.Ref<ReactCodeMirrorRef>}
      aria-label={ariaLabel}
      value={value}
      onChange={onChange}
      extensions={extensions}
      height={height}
    />
  );
});
