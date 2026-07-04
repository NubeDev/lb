// The base CodeMirror JS/JSX editor (widget-builder Slice B) — ported from rubix-cube's
// `manage-template-dialog/code-editor.tsx`, with its REST/react-hook-form data layer removed: it is a
// pure controlled component (`value`/`onChange`) wired to the builder's local state, not a form
// `Controller`. `javascript({ jsx: true })` gives JSX/JS highlighting for the inline `template` body
// (the eval-free HTML/`{{path}}` interpolator), the Plot/D3 snippets, and anything else that edits a
// code string. The theme adapts to the shell's light/dark mode via `useCodeMirrorTheme`.
//
// It edits a code STRING and nothing else — it holds no data and no token. A Plot/D3 snippet runs ONLY
// in the sandboxed iframe; a `template` body is sanitized (`sanitizeTemplateHtml`) and rendered in-process
// by `TemplateView`. One responsibility per file (FILE-LAYOUT).

import { javascript } from "@codemirror/lang-javascript";
import CodeMirror from "@uiw/react-codemirror";

import { useCodeMirrorTheme } from "./theme";

interface Props {
  /** The current snippet string. */
  value: string;
  /** Called with the new snippet on every edit. */
  onChange: (value: string) => void;
  /** Placeholder shown when empty. */
  placeholder?: string;
  /** Accessible label (the builder gives each editor a role-specific one). */
  ariaLabel?: string;
  /** Editor height (CodeMirror height string). */
  height?: string;
}

/** A JS/JSX CodeMirror editor for an inline snippet (the `template` HTML/`{{path}}` body, Plot/D3 code).
 *  Uses the shell's existing `@codemirror/lang-javascript` grammar + a theme that tracks light/dark. */
export function CodeEditor({
  value,
  onChange,
  placeholder,
  ariaLabel = "widget code",
  height = "120px",
}: Props) {
  const cm = useCodeMirrorTheme();
  return (
    <div
      className="overflow-hidden rounded-md border border-border bg-bg"
      aria-label={ariaLabel}
    >
      <CodeMirror
        value={value}
        onChange={onChange}
        placeholder={placeholder}
        extensions={[javascript({ jsx: true }), ...cm.extensions]}
        theme={cm.theme}
        height={height}
        basicSetup={{ lineNumbers: false, foldGutter: false }}
        className="text-xs"
      />
    </div>
  );
}
