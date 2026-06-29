// RulesView — the rules workbench page (rules-workbench + rules-editor-ux scopes): a guided three-region
// Playground. Left: a RuleRail of saved rules. Center: the CodeMirror editor + the RunResult. Right: the
// AuthoringPanel (Functions | Examples | Data) — a discoverable surface that click-to-inserts snippets at
// the editor cursor and loads ready-to-run examples. All state + the real `invoke` calls live in
// `useRules`; this is the layout + the insert wiring. One component per file (FILE-LAYOUT).

import { useRef } from "react";

import { useRules } from "./useRules";
import { RuleRail } from "./RuleRail";
import { RuleEditor } from "./RuleEditor";
import { RunResult } from "./RunResult";
import { AuthoringPanel } from "./panel/AuthoringPanel";
import type { CodeEditorHandle } from "@/components/codeeditor";

interface RulesViewProps {
  ws: string;
}

export function RulesView({ ws }: RulesViewProps) {
  const r = useRules(ws);
  const editorRef = useRef<CodeEditorHandle>(null);

  const insert = (snippet: string) => editorRef.current?.insertSnippet(snippet);

  return (
    <div aria-label="rules workbench" className="flex h-full">
      <RuleRail
        roster={r.roster}
        selectedId={r.selectedId}
        onOpen={r.open}
        onDelete={r.remove}
        onNew={r.newRule}
      />
      <div className="flex min-w-0 flex-1 flex-col">
        <RuleEditor
          ref={editorRef}
          body={r.buffer}
          onChange={r.setBuffer}
          onRun={r.run}
          onSave={r.save}
          selectedId={r.selectedId}
          dirty={r.dirty}
          running={r.running}
        />
        <div className="max-h-[45%] overflow-auto border-t border-border p-3">
          <RunResult result={r.result} error={r.error} running={r.running} />
        </div>
      </div>
      <AuthoringPanel ws={ws} onInsert={insert} onLoadExample={r.loadExample} />
    </div>
  );
}
