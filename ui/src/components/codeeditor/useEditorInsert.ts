// The insert-at-cursor primitive — the one reusable way every palette/explorer panel drops a snippet
// into a CodeMirror editor (rules-editor-ux scope). It holds the live `EditorView` (via the
// `@uiw/react-codemirror` ref) and dispatches a REAL CodeMirror transaction (`replaceSelection`) so the
// controlled `onChange` fires and the buffer updates — never a string concat that fights the controlled
// value. One hook per file (FILE-LAYOUT). Used by `CodeEditor`'s ref handle.

import { useCallback, useRef } from "react";
import type { ReactCodeMirrorRef } from "@uiw/react-codemirror";

export interface EditorInsert {
  /** The ref to pass to `<CodeMirror ref=… />`. */
  ref: React.RefObject<ReactCodeMirrorRef | null>;
  /** Insert `text` at the cursor (replacing any selection); append to the end if no view is focused. */
  insertSnippet: (text: string) => void;
}

/** Hold the editor view and expose an `insertSnippet` that dispatches a real CM transaction. */
export function useEditorInsert(): EditorInsert {
  const ref = useRef<ReactCodeMirrorRef | null>(null);

  const insertSnippet = useCallback((text: string) => {
    const view = ref.current?.view;
    if (!view) return;
    // Replace the current selection (an empty selection = the cursor) with the snippet, then place the
    // cursor at the end of the inserted text and refocus — the real edit path the buffer observes.
    const insertAt = view.state.selection.main;
    view.dispatch({
      changes: { from: insertAt.from, to: insertAt.to, insert: text },
      selection: { anchor: insertAt.from + text.length },
    });
    view.focus();
  }, []);

  return { ref, insertSnippet };
}
