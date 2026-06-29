// RuleEditor — the CodeMirror editor + Run/Save buttons + a dirty indicator (rules-workbench scope).
// Rhai is JS-like, so `lang-javascript` highlighting is good enough (the shipped dep; no Monaco). The
// run uses the BUFFER (ad-hoc body); save persists it. One component per file (FILE-LAYOUT).

import { forwardRef, useState } from "react";
import { javascript } from "@codemirror/lang-javascript";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { CodeEditor, type CodeEditorHandle } from "@/components/codeeditor";

interface RuleEditorProps {
  body: string;
  onChange: (body: string) => void;
  onRun: () => void;
  onSave: (id: string, name: string) => void;
  selectedId: string | null;
  dirty: boolean;
  running: boolean;
}

// `forwardRef` exposes the editor's `insertSnippet` handle so the authoring panel can drop a snippet at
// the cursor (the click-to-insert primitive).
export const RuleEditor = forwardRef<CodeEditorHandle, RuleEditorProps>(function RuleEditor(
  { body, onChange, onRun, onSave, selectedId, dirty, running },
  editorRef,
) {
  const [saveOpen, setSaveOpen] = useState(false);
  const [id, setId] = useState("");
  const [name, setName] = useState("");

  function submitSave() {
    const targetId = selectedId ?? id.trim();
    if (!targetId) return;
    onSave(targetId, name.trim() || targetId);
    setSaveOpen(false);
    setId("");
    setName("");
  }

  return (
    <div className="flex flex-1 flex-col">
      <div className="flex items-center gap-2 border-b border-border px-3 py-2">
        <Button aria-label="run rule" size="sm" disabled={running} onClick={onRun}>
          Run
        </Button>
        {selectedId ? (
          <Button
            aria-label="save rule"
            size="sm"
            variant="outline"
            onClick={() => onSave(selectedId, selectedId)}
          >
            Save
          </Button>
        ) : (
          <Button
            aria-label="save as rule"
            size="sm"
            variant="outline"
            onClick={() => setSaveOpen((v) => !v)}
          >
            Save as…
          </Button>
        )}
        {dirty ? (
          <span aria-label="dirty indicator" className="text-xs text-amber-600">
            ● unsaved
          </span>
        ) : null}
      </div>

      {saveOpen && !selectedId ? (
        <div className="flex items-center gap-2 border-b border-border bg-muted px-3 py-2">
          <Input
            aria-label="new rule id"
            placeholder="rule id"
            value={id}
            onChange={(e) => setId(e.target.value)}
          />
          <Input
            aria-label="new rule name"
            placeholder="name (optional)"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
          <Button aria-label="confirm save rule" size="sm" onClick={submitSave}>
            Save
          </Button>
        </div>
      ) : null}

      <div className="flex-1 overflow-auto">
        <CodeEditor
          ref={editorRef}
          ariaLabel="rule editor"
          value={body}
          onChange={onChange}
          extensions={[javascript()]}
          height="100%"
        />
      </div>
    </div>
  );
});
