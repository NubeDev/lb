// RulesView — the rules workbench page (rules-workbench + rules-editor-ux + rules-workflow-convergence
// scopes). A two-tab surface: the Editor (the three-region Playground — RuleRail + AppPageHeader over
// the CodeMirror editor + run result, with the AuthoringPanel on the right) and Workflows (the
// rule-author view over the flows engine — pick WHEN a rule runs, the host stores it as a flow).
//
// The page follows the canonical tabbed-surface shape (the same one AdminView uses): an
// `<AppPageHeader>`-led `<section>` with `<Tabs>` below. All editor state + the real `invoke` calls
// live in `useRules`; all workflow state lives in `useRuleWorkflows`; this is the layout, the insert
// wiring, the ⌘S/inline-name save flow, and the tab switch. One component per file (FILE-LAYOUT).

import { useEffect, useRef, useState, type FormEvent } from "react";
import { FileCode2, Pencil, Play, Save, WandSparkles } from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { CollapsedRail } from "@/components/app/rail-collapsed";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useRules } from "./useRules";
import { useAutoFormat } from "./useAutoFormat";
import { RuleRail } from "./RuleRail";
import { RuleEditor } from "./RuleEditor";
import { formatRhai } from "./formatRhai";
import { ResultBar, type ResultView } from "./ResultBar";
import { RunResult } from "./RunResult";
import { AuthoringPanel } from "./panel/AuthoringPanel";
import { RulesWorkflowsTab } from "./RulesWorkflowsTab";
import type { CodeEditorHandle } from "@/components/codeeditor";
import { useVerticalSplit, SplitHandle } from "@/lib/split";

type Tab = "editor" | "workflows";

interface RulesViewProps {
  ws: string;
  /** The rule id from the URL (`/rules/$rule`), or null on the bare `/rules` (fresh buffer). */
  ruleId?: string | null;
  /** Reflect the open rule in the URL — id to open `/rules/$rule`, null for the bare `/rules`. */
  onSelectRule?: (id: string | null) => void;
}

export function RulesView({ ws, ruleId = null, onSelectRule }: RulesViewProps) {
  const r = useRules(ws);
  const autoFormat = useAutoFormat();
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

  // A deep link to a specific rule forces the editor tab — a user clicking `/rules/some-rule` from
  // outside (a dashboard link, a deep-link email) expects to land on the rule, not the workflows list.
  useEffect(() => {
    if (ruleId) setTab("editor");
  }, [ruleId]);

  // One inline name field serves both flows: rename an open rule, or name-on-first-save an ad-hoc
  // buffer. `mode` says which — so ⌘S on an unsaved buffer opens it in "save" mode instead of failing.
  const [nameField, setNameField] = useState<null | "rename" | "save">(null);
  // The rule rail folds to the shared thin strip (same affordance as the dashboard roster).
  const [railOpen, setRailOpen] = useState(true);
  const [nameValue, setNameValue] = useState("");
  // Table (typed views) vs. JSON (verbatim) — the result-region view toggle, owned here so the
  // ResultBar toggle and the RunResult body stay in sync.
  const [resultView, setResultView] = useState<ResultView>("table");
  // The page tab: the rules editor (default) or the workflows list. A deep link to a specific rule
  // (`/rules/$rule`) forces the editor so the open rule is the thing the user lands on, not the
  // workflows roster.
  const [tab, setTab] = useState<Tab>("editor");
  // A draggable divider between the CodeMirror editor (top) and the run-result region (bottom):
  // the editor is the larger half by default; drag up to grow the result, down to grow the editor.
  const split = useVerticalSplit(0.7);

  const insert = (snippet: string) => editorRef.current?.insertSnippet(snippet);
  const selectedName = r.name ?? null;

  // Reformat the buffer in place. `formatRhai` is idempotent, so a re-format on an already-tidy
  // buffer is a no-op (no spurious dirty flip). Shared by the manual Format button and the
  // auto-format-on-blur path.
  function formatBuffer() {
    const next = formatRhai(r.buffer);
    if (next !== r.buffer) r.setBuffer(next);
  }
  // Auto-format fires when the editor loses focus (a settle point — never mid-keystroke), and only
  // when the user has toggled it on. Blur also precedes any Run/Save, so those read the tidy buffer.
  const onEditorBlur = autoFormat.enabled ? formatBuffer : undefined;

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
    <section aria-label="rules workbench" className="flex h-full min-w-0 flex-col bg-bg text-fg">
      <AppPageHeader
        icon={FileCode2}
        title={selectedName ?? r.selectedId ?? "Untitled rule"}
        description="Author Rhai rules and wire them into workflows that run on a schedule or event."
        workspace={ws}
        actions={
          <>
            <Button aria-label="run rule" size="sm" disabled={r.running} onClick={() => void r.run()}>
              <Play size={14} /> {r.running ? "Running…" : "Run"}
            </Button>
            <Button
              aria-label="format rule code"
              size="sm"
              variant="ghost"
              disabled={!r.buffer.trim()}
              onClick={formatBuffer}
            >
              <WandSparkles size={14} /> Format
            </Button>
            {/* Auto-format toggle — persisted in localStorage (browser-wide), so the choice sticks
                across reloads. When on, the buffer is tidied whenever the editor loses focus. */}
            <label className="flex cursor-pointer items-center gap-1.5 text-xs text-muted">
              <Switch
                aria-label="auto-format on blur"
                checked={autoFormat.enabled}
                onCheckedChange={autoFormat.toggle}
              />
              Auto
            </label>
            {r.selectedId ? (
              <Button aria-label="rename rule" size="sm" variant="ghost" onClick={openRename}>
                <Pencil size={14} /> Rename
              </Button>
            ) : null}
            {/* Save is ALWAYS present — the fix for "I can't save a rule". Unopened buffers save-as.
                Visible on the editor tab; the actions row is page-wide so it remains reachable. */}
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
      />
      <Tabs
        value={tab}
        onValueChange={(v) => setTab(v as Tab)}
        className="min-h-0 flex-1 flex flex-col"
      >
        <TabsList className="m-2 self-start">
          <TabsTrigger value="editor" aria-label="editor tab">
            Editor
          </TabsTrigger>
          <TabsTrigger value="workflows" aria-label="workflows tab">
            Workflows
          </TabsTrigger>
        </TabsList>

        {/* The body is rendered directly (not via TabsContent) — TabsContent wraps children in <Reveal>,
            which collapses the editor's flex-1 sizing chain and breaks the CodeMirror region. The
            Studio section uses the same shape. The `tab` state already owns which panel is shown. */}
        <div className="min-h-0 flex-1 overflow-hidden">
          {tab === "editor" ? (
            <EditorBody
              railOpen={railOpen}
              onCollapseRail={() => setRailOpen(false)}
              onExpandRail={() => setRailOpen(true)}
              rules={r}
              nameField={nameField}
              nameValue={nameValue}
              setNameValue={setNameValue}
              submitName={submitName}
              dismissName={() => setNameField(null)}
              ws={ws}
              insert={insert}
              editorRef={editorRef}
              onEditorBlur={onEditorBlur}
              split={split}
              resultView={resultView}
              setResultView={setResultView}
              onSelectRule={onSelectRule}
            />
          ) : (
            <RulesWorkflowsTab ws={ws} ruleRoster={r.roster} onJumpToRule={(id) => {
              if (onSelectRule) onSelectRule(id);
              setTab("editor");
            }} />
          )}
        </div>
      </Tabs>
    </section>
  );
}

/** The editor body — extracted so the RulesView return stays scannable. Owns no state; everything
 *  comes from the parent (the workbench hook + the inline-name field + the split + the URL sync). */
interface EditorBodyProps {
  railOpen: boolean;
  onCollapseRail: () => void;
  onExpandRail: () => void;
  rules: ReturnType<typeof useRules>;
  nameField: null | "rename" | "save";
  nameValue: string;
  setNameValue: (v: string) => void;
  submitName: (e: FormEvent) => void;
  dismissName: () => void;
  ws: string;
  insert: (snippet: string) => void;
  editorRef: React.RefObject<CodeEditorHandle>;
  /** Fired on editor blur when auto-format is enabled (undefined = auto-format off). */
  onEditorBlur?: () => void;
  split: ReturnType<typeof useVerticalSplit>;
  resultView: ResultView;
  setResultView: (v: ResultView) => void;
  onSelectRule?: (id: string | null) => void;
}

function EditorBody({
  railOpen,
  onCollapseRail,
  onExpandRail,
  rules: r,
  nameField,
  nameValue,
  setNameValue,
  submitName,
  dismissName,
  ws,
  insert,
  editorRef,
  onEditorBlur,
  split,
  resultView,
  setResultView,
  onSelectRule,
}: EditorBodyProps) {
  return (
    <div className="flex h-full min-h-0 min-w-0">
      {railOpen ? (
        <RuleRail
          roster={r.roster}
          selectedId={r.selectedId}
          // Navigate to the rule's URL; the effect above opens it. Falls back to a direct open when
          // this view isn't URL-driven (e.g. an embedded/test render with no `onSelectRule`).
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
          onCollapse={onCollapseRail}
        />
      ) : (
        <CollapsedRail noun="rule" onExpand={onExpandRail} />
      )}

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
                if (e.key === "Escape") dismissName();
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
              onClick={dismissName}
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
              <RuleEditor
                ref={editorRef}
                body={r.buffer}
                onChange={r.setBuffer}
                onBlur={onEditorBlur}
              />
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
          <AuthoringPanel
            ws={ws}
            onInsert={insert}
            onLoadExample={r.loadExample}
            params={r.params}
            onParamsChange={r.setParams}
          />
        </div>
      </div>
    </div>
  );
}
