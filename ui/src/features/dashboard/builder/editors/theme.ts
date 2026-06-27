// The shared CodeMirror theme + base extensions for the in-app scripted-view editors (widget-builder
// Slice B). rubix-cube's editors import a shared `theme` + `EditorView.lineWrapping`; we provide the
// lazybones equivalent — a minimal, token-bound `EditorView.theme` (so the editor matches the shell's
// dark surface) plus line-wrapping, used by every editor (JSX/Plot/template/SQL). One responsibility
// per file (FILE-LAYOUT): this is only the look; the language extensions live with each editor.
//
// The editor is a TRUSTED-SHELL authoring surface — it edits a code STRING. The string still executes
// ONLY in the sandboxed iframe (or trusted-key in-process), never here. Editing ≠ running.

import { EditorView } from "@codemirror/view";

/** A compact, surface-matching CodeMirror theme bound to the shell's CSS variables (the same tokens the
 *  `FIELD` inputs use). Transparent background so it sits on the panel; small monospace text. */
export const editorTheme = EditorView.theme(
  {
    "&": {
      fontSize: "12px",
      backgroundColor: "transparent",
      color: "var(--fg, inherit)",
    },
    ".cm-content": {
      fontFamily:
        "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
      caretColor: "var(--accent, currentColor)",
    },
    ".cm-gutters": {
      backgroundColor: "transparent",
      color: "var(--muted, #888)",
      border: "none",
    },
    "&.cm-focused": { outline: "none" },
    ".cm-activeLine": { backgroundColor: "color-mix(in srgb, var(--accent, #888) 8%, transparent)" },
    ".cm-activeLineGutter": { backgroundColor: "transparent" },
  },
  { dark: true },
);

/** Line-wrapping — every editor wants long snippets/queries to wrap rather than scroll horizontally. */
export const lineWrapping = EditorView.lineWrapping;

/** The base extension set shared by all editors (theme + wrapping). Language modes are added per editor. */
export const baseExtensions = [editorTheme, lineWrapping];
