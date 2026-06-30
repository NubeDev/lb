// RuleEditor — the CodeMirror editor surface (rules-workbench scope). Rhai is JS-like, so
// `lang-javascript` highlighting is good enough (the shipped dep; no Monaco). The run/save
// controls live in the page header (`RulesView`); this is the editor only. `forwardRef` exposes
// the `insertSnippet` handle so the authoring panel can drop a snippet at the cursor. One
// component per file (FILE-LAYOUT).

import { forwardRef } from "react";
import { javascript } from "@codemirror/lang-javascript";

import { CodeEditor, type CodeEditorHandle } from "@/components/codeeditor";

interface RuleEditorProps {
  body: string;
  onChange: (body: string) => void;
}

// `forwardRef` exposes the editor's `insertSnippet` handle so the authoring panel can drop a snippet at
// the cursor (the click-to-insert primitive).
export const RuleEditor = forwardRef<CodeEditorHandle, RuleEditorProps>(function RuleEditor(
  { body, onChange },
  editorRef,
) {
  return (
    <div className="min-h-0 flex-1 overflow-auto">
      <CodeEditor
        ref={editorRef}
        ariaLabel="rule editor"
        value={body}
        onChange={onChange}
        extensions={[javascript()]}
        height="100%"
      />
    </div>
  );
});
