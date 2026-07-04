// The shared CodeMirror theme + base extensions for the in-app scripted-view editors (widget-builder
// Slice B). rubix-cube's editors import a shared `theme` + `EditorView.lineWrapping`; we provide the
// lazybones equivalent — a minimal, token-bound `EditorView.theme` (so the editor chrome matches the
// shell surface via CSS variables) plus line-wrapping, used by every editor (JSX/Plot/template/SQL).
// One responsibility per file (FILE-LAYOUT): this is only the look; the language extensions live with
// each editor.
//
// The editor is a TRUSTED-SHELL authoring surface — it edits a code STRING. The string still executes
// ONLY in the sandboxed iframe (or, for the eval-free `template` engine, is sanitized + rendered
// in-process by `TemplateView`), never here. Editing ≠ running.
//
// Theme: the chrome (background/caret/gutters/active-line) is bound to the shell's CSS variables so it
// matches whatever look the member picked; the SYNTAX highlighting is `@uiw/react-codemirror`'s
// built-in light/dark theme, switched by the shell's CURRENT mode so the code is legible in BOTH light
// and dark (the prior `{ dark: true }` marker was a static hint that didn't actually drive the highlight
// colors — the built-in theme prop does).

import { useMemo } from "react";
import { EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";

import { useThemeOptional } from "@/lib/theme/useTheme";

/** A compact, surface-matching CodeMirror chrome bound to the shell's CSS variables (the same tokens the
 *  `FIELD` inputs use). Transparent background so it sits on the panel; small monospace text. Token colors
 *  come from the `@uiw/react-codemirror` theme picked by {@link useCodeMirrorTheme} (mode-correct). */
export const editorTheme = EditorView.theme({
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
});

/** Line-wrapping — every editor wants long snippets/queries to wrap rather than scroll horizontally. */
export const lineWrapping = EditorView.lineWrapping;

/** The base extension set shared by all editors (chrome theme + wrapping). Language modes + the
 *  mode-correct highlight theme are added per editor via {@link useCodeMirrorTheme}. */
export const baseExtensions: Extension[] = [editorTheme, lineWrapping];

/** The CodeMirror theme info an editor needs: the chrome `extensions` (stable) + the `@uiw/react-codemirror`
 *  highlight theme id (`"dark"` / `"light"`) tracked to the shell's CURRENT mode so the code stays legible
 *  in both. Defaults to dark when rendered outside a `ThemeProvider` (tests / standalone mounts). */
export function useCodeMirrorTheme(): { extensions: Extension[]; theme: "dark" | "light" } {
  const ctx = useThemeOptional();
  const mode = ctx?.theme.mode ?? "dark";
  const extensions = useMemo(() => baseExtensions, []);
  return { extensions, theme: mode === "dark" ? "dark" : "light" };
}
