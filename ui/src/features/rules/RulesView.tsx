// RulesView — the rules workbench page (rules-workbench + rules-editor-ux scopes): a guided
// three-region Playground shaped like the dashboard surfaces. Left: a RuleRail of saved rules.
// Center: an AppPageHeader (the current rule + Run/Save controls) over the CodeMirror editor and
// the RunResult. Right: the AuthoringPanel (Functions | Examples | Data) — a discoverable surface
// that click-to-inserts snippets at the editor cursor and loads ready-to-run examples. All state +
// the real `invoke` calls live in `useRules`; this is the layout + the insert wiring. One
// component per file (FILE-LAYOUT).

import { useRef, useState, type FormEvent } from "react";
import { FileCode2, Pencil } from "lucide-react";

import { AppPage } from "@/components/app/page";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
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

  // Inline rename state — opens a name field right under the header (contextual to the open rule).
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState("");

  const insert = (snippet: string) => editorRef.current?.insertSnippet(snippet);

  const selectedName = r.name ?? null;

  function startRename() {
    setRenameValue(selectedName ?? r.selectedId ?? "");
    setRenaming(true);
  }

  async function submitRename(e: FormEvent) {
    e.preventDefault();
    const trimmed = renameValue.trim();
    if (!trimmed) return;
    const ok = await r.rename(trimmed);
    if (ok) setRenaming(false);
  }

  return (
    <AppPage
      label="rules workbench"
      icon={FileCode2}
      title={selectedName ?? r.selectedId ?? "Rules"}
      description="Author and run Rhai rules over live workspace data."
      workspace={ws}
      actions={
        <>
          <Button aria-label="run rule" size="sm" disabled={r.running} onClick={() => void r.run()}>
            Run
          </Button>
          {r.selectedId ? (
            <>
              <Button aria-label="rename rule" size="sm" variant="ghost" onClick={startRename}>
                <Pencil size={14} /> Rename
              </Button>
              <Button
                aria-label="save rule"
                size="sm"
                variant="outline"
                onClick={() => {
                  const sid = r.selectedId;
                  if (sid) void r.save(sid, selectedName ?? sid);
                }}
              >
                Save
              </Button>
            </>
          ) : null}
          {r.dirty ? (
            <span aria-label="dirty indicator" className="text-xs text-accent">
              ● unsaved
            </span>
          ) : null}
        </>
      }
    >
      <RuleRail
        roster={r.roster}
        selectedId={r.selectedId}
        onOpen={r.open}
        onDelete={r.remove}
        onCreate={r.create}
      />

      <div className="flex min-w-0 flex-1 flex-col">
        {renaming && r.selectedId ? (
          <form
            aria-label="rename rule form"
            className="flex items-center gap-2 border-b border-border bg-panel px-4 py-2"
            onSubmit={submitRename}
          >
            <Input
              aria-label="rule name"
              autoFocus
              className="h-8"
              placeholder="Rule name"
              value={renameValue}
              onChange={(e) => setRenameValue(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") setRenaming(false);
              }}
            />
            <Button aria-label="confirm rename rule" size="sm" type="submit">
              Save name
            </Button>
            <Button
              aria-label="cancel rename rule"
              size="sm"
              variant="ghost"
              type="button"
              onClick={() => setRenaming(false)}
            >
              Cancel
            </Button>
          </form>
        ) : null}

        <div className="flex min-h-0 flex-1">
          <div className="flex min-w-0 flex-1 flex-col">
            <RuleEditor ref={editorRef} body={r.buffer} onChange={r.setBuffer} />
            <div className="max-h-[45%] overflow-auto border-t border-border p-3">
              <RunResult result={r.result} error={r.error} running={r.running} />
            </div>
          </div>
          <AuthoringPanel ws={ws} onInsert={insert} onLoadExample={r.loadExample} />
        </div>
      </div>
    </AppPage>
  );
}
