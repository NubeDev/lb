// The base CodeMirror JSX/JS editor (widget-builder Slice B) — ported from rubix-cube's
// `manage-template-dialog/code-editor.tsx`, with its REST/react-hook-form data layer removed: it is a
// pure controlled component (`value`/`onChange`) wired to the builder's local state, not a form
// `Controller`. `javascript({ jsx: true })` gives JSX/Plot/D3/template highlighting; `lineWrapping` +
// the shared theme match the shipped shell surface.
//
// It edits a code STRING and nothing else — it holds no data and no token. The string runs ONLY in the
// sandboxed iframe (the v2 trust contract is unchanged): authored in the trusted shell, executed
// sandboxed. One responsibility per file (FILE-LAYOUT).

import { javascript } from "@codemirror/lang-javascript";
import CodeMirror from "@uiw/react-codemirror";

import { baseExtensions } from "./theme";

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

/** A JSX/JS CodeMirror editor for an inline scripted-view snippet (Plot/D3/template code). */
export function CodeEditor({
  value,
  onChange,
  placeholder,
  ariaLabel = "widget code",
  height = "120px",
}: Props) {
  return (
    <div
      className="overflow-hidden rounded-md border border-border bg-bg"
      aria-label={ariaLabel}
    >
      <CodeMirror
        value={value}
        onChange={onChange}
        placeholder={placeholder}
        extensions={[javascript({ jsx: true }), ...baseExtensions]}
        height={height}
        basicSetup={{ lineNumbers: false, foldGutter: false }}
        className="text-xs"
      />
    </div>
  );
}
