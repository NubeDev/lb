// RulesView — the rules workbench page (rules-workbench + rules-editor-ux scopes): a guided
// three-region Playground shaped like the dashboard surfaces. Left: a RuleRail of saved rules.
// Center: an AppPageHeader (Run + an ALWAYS-available Save) over the CodeMirror editor and the run
// result (a ResultBar status header over the RunResult). Right: the AuthoringPanel (Functions |
// Examples | Data). All state + the real `invoke` calls live in `useRules`; this is the layout, the
// insert wiring, the ⌘S/inline-name save flow. One component per file (FILE-LAYOUT).

import { useEffect, useRef, useState, type FormEvent } from "react";
import { FileCode2, Pencil, Play, Save } from "lucide-react";

import { AppPage } from "@/components/app/page";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useRules } from "./useRules";
import { RuleRail } from "./RuleRail";
import { RuleEditor } from "./RuleEditor";
import { ResultBar, type ResultView } from "./ResultBar";
import { RunResult } from "./RunResult";
import { AuthoringPanel } from "./panel/AuthoringPanel";
import type { CodeEditorHandle } from "@/components/codeeditor";
import { useVerticalSplit, SplitHandle } from "@/lib/split";

interface RulesViewProps {
  ws: string;
  /** The rule id from the URL (`/rules/$rule`), or null on the bare `/rules` (fresh buffer). */
  ruleId?: string | null;
  /** Reflect the open rule in the URL — id to open `/rules/$rule`, null for the bare `/rules`. */
  onSelectRule?: (id: string | null) => void;
}

export function RulesView({ ws, ruleId = null, onSelectRule }: RulesViewProps) {
  const r = useRules(ws);
  const editorRef = useRef<CodeEditorHandle>(null);

  // The URL is the source of truth for which rule is open. When `ruleId` changes (deep link, back/
  // forward, or a select that navigated), load it — or reset to a fresh buffer for the bare route.
  // Guard on the currently-open id so this doesn't clobber an in-progress edit on unrelated re-renders.
  useEffect(() => {
    if (ruleId) {
      if (ruleId !== r.selectedId) void r.open(ruleId);
    } else if (r.selectedId !== null) {
      r.newRule();
    }
    // Only react to the URL param; `r` is stable enough per-render for this sync.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ruleId]);

  // One inline name field serves both flows: rename an open rule, or name-on-first-save an ad-hoc
  // buffer. `mode` says which — so ⌘S on an unsaved buffer opens it in "save" mode instead of failing.
  const [nameField, setNameField] = useState<null | "rename" | "save">(null);
  const [nameValue, setNameValue] = useState("");
  // Table (typed views) vs. JSON (verbatim) — the result-region view toggle, owned here so the
  // ResultBar toggle and the RunResult body stay in sync.
  const [resultView, setResultView] = useState<ResultView>("table");
  // A draggable divider between the CodeMirror editor (top) and the run-result region (bottom):
  // the editor is the larger half by default; drag up to grow the result, down to grow the editor.
  const split = useVerticalSplit(0.7);

  const insert = (snippet: string) => editorRef.current?.insertSnippet(snippet);
  const selectedName = r.name ?? null;

  function openRename() {
    setNameValue(selectedName ?? r.selectedId ?? "");
    setNameField("rename");
  }

  // Save: an open rule updates in place; a fresh buffer needs a name, so we reveal the name field.
  async function doSave() {
    const res = await r.saveCurrent();
    if (res.needsName) {
      setNameValue("");
      setNameField("save");
    }
  }

  // After a save/create lands on a saved rule, reflect its id in the URL if the URL doesn't already
  // carry it (name-first Save on a fresh buffer, or a ⌘S that promoted a buffer to a saved rule).
  function syncUrlToOpen() {
    if (onSelectRule && r.selectedId && r.selectedId !== ruleId) onSelectRule(r.selectedId);
  }

  // ⌘S / Ctrl+S saves the current rule from anywhere on the page (the reflex a workbench must honour).
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        void doSave();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // doSave closes over r.saveCurrent; r is stable enough per-render for this handler.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [r.selectedId, r.name, r.buffer]);

  async function submitName(e: FormEvent) {
    e.preventDefault();
    const trimmed = nameValue.trim();
    if (!trimmed) return;
    if (nameField === "rename") {
      if (await r.rename(trimmed)) setNameField(null);
    } else {
      const res = await r.saveCurrent(trimmed);
      if (res.ok) {
        setNameField(null);
        syncUrlToOpen();
      }
    }
  }

  const saveLabel = r.selectedId ? (r.dirty ? "Save" : "Saved") : "Save";

  return (
    <AppPage
      label="rules workbench"
      icon={FileCode2}
      title={selectedName ?? r.selectedId ?? "Untitled rule"}
      description="Author and run Rhai rules over live workspace data."
      workspace={ws}
      actions={
        <>
          <Button aria-label="run rule" size="sm" disabled={r.running} onClick={() => void r.run()}>
            <Play size={14} /> {r.running ? "Running…" : "Run"}
          </Button>
          {r.selectedId ? (
            <Button aria-label="rename rule" size="sm" variant="ghost" onClick={openRename}>
              <Pencil size={14} /> Rename
            </Button>
          ) : null}
          {/* Save is ALWAYS present — the fix for "I can't save a rule". Unopened buffers save-as. */}
          <Button
            aria-label="save rule"
            size="sm"
            variant={r.dirty || !r.selectedId ? "default" : "outline"}
            disabled={r.selectedId != null && !r.dirty}
            onClick={() => void doSave()}
          >
            <Save size={14} /> {saveLabel}
          </Button>
          {r.dirty ? (
            <span aria-label="dirty indicator" className="text-xs font-medium text-accent">
              ● Unsaved
            </span>
          ) : null}
        </>
      }
    >
      <RuleRail
        roster={r.roster}
        selectedId={r.selectedId}
        // Navigate to the rule's URL; the effect above opens it. Falls back to a direct open when this
        // view isn't URL-driven (e.g. an embedded/test render with no `onSelectRule`).
        onOpen={(id) => (onSelectRule ? onSelectRule(id) : void r.open(id))}
        onDelete={async (id) => {
          await r.remove(id);
          // Deleting the open rule drops back to the bare `/rules` URL (the buffer was reset).
          if (onSelectRule && id === r.selectedId) onSelectRule(null);
        }}
        onCreate={async (name) => {
          const id = await r.create(name);
          if (id && onSelectRule) onSelectRule(id);
          return id;
        }}
      />

      <div className="flex min-w-0 flex-1 flex-col">
        {nameField ? (
          <form
            aria-label={nameField === "rename" ? "rename rule form" : "save rule form"}
            className="flex items-center gap-2 border-b border-border bg-panel px-4 py-2"
            onSubmit={submitName}
          >
            <label className="text-xs font-medium text-muted" htmlFor="rule-name-field">
              {nameField === "rename" ? "Rename to" : "Save as"}
            </label>
            <Input
              id="rule-name-field"
              aria-label="rule name"
              autoFocus
              className="h-8 max-w-xs"
              placeholder="Rule name"
              value={nameValue}
              onChange={(e) => setNameValue(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") setNameField(null);
              }}
            />
            <Button
              aria-label={nameField === "rename" ? "confirm rename rule" : "confirm save rule name"}
              size="sm"
              type="submit"
              disabled={!nameValue.trim()}
            >
              {nameField === "rename" ? "Save name" : "Save rule"}
            </Button>
            <Button
              aria-label="cancel save rule name"
              size="sm"
              variant="ghost"
              type="button"
              onClick={() => setNameField(null)}
            >
              Cancel
            </Button>
          </form>
        ) : null}

        <div className="flex min-h-0 flex-1">
          <div ref={split.containerRef} className="flex min-w-0 flex-1 flex-col">
            <div
              className="flex min-h-[6rem] shrink-0 flex-col overflow-hidden"
              style={{ flexBasis: split.topBasis, pointerEvents: split.dragging ? "none" : undefined }}
            >
              <RuleEditor ref={editorRef} body={r.buffer} onChange={r.setBuffer} />
            </div>
            <SplitHandle onPointerDown={split.onHandleDown} label="resize editor and result" />
            <div
              className="flex min-h-[6rem] flex-1 flex-col"
              style={{ pointerEvents: split.dragging ? "none" : undefined }}
            >
              <ResultBar
                result={r.result}
                error={r.error}
                running={r.running}
                hasRun={r.hasRun}
                view={resultView}
                onViewChange={setResultView}
              />
              <div className="min-h-0 flex-1 overflow-auto p-3">
                <RunResult
                  result={r.result}
                  error={r.error}
                  running={r.running}
                  hasRun={r.hasRun}
                  view={resultView}
                />
              </div>
            </div>
          </div>
          <AuthoringPanel ws={ws} onInsert={insert} onLoadExample={r.loadExample} />
        </div>
      </div>
    </AppPage>
  );
}
