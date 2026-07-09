// A reusable controlled code editor with an imperative insert-at-cursor handle (rules-editor-ux scope).
// Wraps `@uiw/react-codemirror` and exposes `insertSnippet(text)` via a ref so any sibling panel (a
// function palette, a data explorer) can drop a snippet at the cursor — the click-to-insert primitive,
// extracted so a future editor surface reuses it rather than re-wiring CodeMirror. The caller supplies
// the language extension (Rhai → `lang-javascript`, SurrealQL → `lang-sql`), so this stays language-
// neutral. One component per file (FILE-LAYOUT).

import { forwardRef, useContext, useImperativeHandle } from "react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import type { Extension } from "@codemirror/state";

import { ThemeContext } from "@/lib/theme/theme-context";

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
  /** Fired when the editor loses focus — a natural, non-disruptive moment to auto-format. */
  onBlur?: () => void;
  /** Read-only mode: syntax-highlighted, selectable, but not editable (a preview surface). Defaults
   *  to `true` (editable). When `false`, CodeMirror keeps highlighting + selection but blocks input and
   *  hides the caret — the reuse seam for "show this code, don't let me change it" (e.g. a wizard that
   *  previews a canonical rule the user only runs). */
  editable?: boolean;
}

/** A controlled CodeMirror editor exposing an `insertSnippet` ref handle. */
export const CodeEditor = forwardRef<CodeEditorHandle, CodeEditorProps>(function CodeEditor(
  { value, onChange, extensions, height = "100%", ariaLabel = "code editor", onBlur, editable = true },
  handleRef,
) {
  const { ref, insertSnippet } = useEditorInsert();
  useImperativeHandle(handleRef, () => ({ insertSnippet }), [insertSnippet]);

  // Follow the app's light/dark preference (the `.dark` class on <html>). Without this CodeMirror pins
  // its built-in light theme, so the editor stays white on the shell's dark surface. Read the context
  // directly (not the throwing `useTheme` hook) so this shared editor also renders outside a
  // ThemeProvider (e.g. isolated tests), defaulting to light.
  const mode = useContext(ThemeContext)?.theme.mode ?? "light";

  return (
    <CodeMirror
      ref={ref as React.Ref<ReactCodeMirrorRef>}
      aria-label={ariaLabel}
      value={value}
      onChange={onChange}
      onBlur={onBlur}
      extensions={extensions}
      theme={mode}
      height={height}
      editable={editable}
    />
  );
});
