// The render-template widget wizard (setup scope) — a guided path from the seeded buildings datasource
// to a saved, reusable render-template widget the user designs against their real query rows. It is the
// sibling of the data→insight wizard (`DatasourceWizard`) and shares its first two steps VERBATIM
// (`steps/DatasourceStep` + `steps/SqlPreviewStep`, the rule-3 extraction); the tail swaps insight for
// a template widget. PURE ORCHESTRATION over shipped pieces:
//
//   1. Datasource — the real `datasource.list`/`datasource.add` roster (shared `DatasourceStep`).
//   2. Query      — the preloaded, read-only `DEMO_SQL` run through the Query workbench engine (shared
//                   `SqlPreviewStep`); its rows feed the widget preview + the AI prompt.
//   3. Design     — the real `TemplateSourceField` (inline HTML/JSX editor) beside a live `WidgetHost`
//                   preview of a `view:"template"` cell (`templateCell`) bound to the same query.
//   4. Ask an AI  — the shipped `CopyTemplatePrompt`: the engine contract + the user's real rows + the
//                   SQL, one clipboard paste, so any external agent can return paste-ready template HTML.
//   5. Save       — the real `template.save` (`saveTemplate`) persists a durable `render_template`;
//                   optionally the same cell is dropped on a fresh dashboard via `dashboard.save`.
//
// No new backend, no new verb, no duplicated editor. New code is this flow + the shared `templateCell`
// / starter gallery in `@/lib/panel` (lifted there so the reports PanelPicker shares them without a
// cross-feature import). Cap-gating hides
// controls the caller couldn't use anyway;
// the gateway re-checks every write (rule 5). Datasource/source ids stay opaque (rule 10). One
// responsibility per file (FILE-LAYOUT).

import { useEffect, useMemo, useState } from "react";
import {
  Check,
  Code2,
  Database,
  EyeOff,
  LayoutDashboard,
  LayoutTemplate,
  Loader2,
  ScrollText,
  Sparkles,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { CAP, hasCap } from "@/lib/session";
import { saveDashboard } from "@/lib/dashboard";
import { saveTemplate } from "@/lib/dashboard/template.api";
import { WidgetHost } from "@/features/dashboard/WidgetHost";
import { ResultRowsProvider } from "@/features/panel-builder/fields/RowsContext";
import { CopyTemplatePrompt } from "@/features/panel-builder/CopyTemplatePrompt";
import {
  TemplateSourceField,
  type TemplateValue,
} from "@/features/dashboard/builder/editors/TemplateSourceField";
import { StepFlow, StepShell, type FlowStep } from "../wizard/StepFlow";
import { DatasourceStep } from "./steps/DatasourceStep";
import { SqlPreviewStep } from "./steps/SqlPreviewStep";
import { DEFAULT_SOURCE, templateCell } from "./dataToInsight";
import { DEFAULT_TEMPLATE, TEMPLATE_GALLERY, type TemplateExample } from "@/lib/panel";

type Row = Record<string, unknown>;

interface Props {
  ws: string;
  caps: string[] | undefined;
  onDone?: () => void;
}

export function TemplateWidgetWizard({ ws, caps, onDone }: Props) {
  const canRunQuery = hasCap(caps, CAP.datasourceList); // reading a source ≈ the workbench nav gate
  // template.save + dashboard.save share the widget-authoring trust class (render-template scope); the
  // caps map exposes only `dashboard.save`, so it gates BOTH the durable save and the dashboard drop.
  const canSave = hasCap(caps, CAP.dashboardSave);

  // Accumulated across steps: the chosen datasource, the picked starter example (drives BOTH the query
  // SQL and the initial template code), the ran rows (feed preview + prompt), the template HTML the user
  // is designing, and where a saved template/dashboard landed. Each gallery example ships its own summary
  // SQL, so switching examples re-points the query the widget renders.
  const [source, setSource] = useState(DEFAULT_SOURCE);
  const [example, setExample] = useState<TemplateExample>(DEFAULT_TEMPLATE);
  const [rows, setRows] = useState<Row[]>([]);
  const [code, setCode] = useState(DEFAULT_TEMPLATE.code);
  const sql = example.sql;

  /** Pick a starter example — swaps the query SQL AND resets the editor to that example's code (so the
   *  preview + AI prompt track the new shape). Re-picking the active one is a no-op. */
  const pickExample = (next: TemplateExample) => {
    if (next.id === example.id) return;
    setExample(next);
    setCode(next.code);
    setRows([]); // the previous run's rows are the old shape — re-run against the new SQL
  };

  const steps: FlowStep[] = [
    {
      key: "intro",
      label: "Overview",
      hint: "The whole path",
      render: () => (
        <StepShell
          icon={Sparkles}
          title="Build a render-template widget"
          blurb="A render-template widget turns query rows into a custom HTML panel — no charting library, just your own markup bound to the data. This walks you from the seeded buildings datasource, through the query, to designing the widget — start from a polished example or draft one with AI and preview it live — then saves it as a reusable template."
        >
          <ol className="space-y-2">
            <IntroItem icon={Database} n={1} title="Datasource — where the data lives" body="A registered connection (here the SQLite buildings dataset). The query reads through it." />
            <IntroItem icon={ScrollText} n={2} title="Query — the rows to render" body="Run a preloaded query and see the rows. Your widget will render exactly this shape." />
            <IntroItem icon={Code2} n={3} title="Design — build & preview the widget" body="Start from an example or draft one with AI (copy the prompt), edit the HTML, and preview it live against your real rows." />
            <IntroItem icon={LayoutTemplate} n={4} title="Save — a reusable widget" body="Persist it as a durable template you can drop into any panel, and optionally onto a new dashboard." />
          </ol>
        </StepShell>
      ),
    },
    {
      key: "datasource",
      label: "Datasource",
      hint: "Where data lives",
      render: () => (
        <StepShell
          icon={Database}
          title="Pick the datasource"
          blurb="A datasource is a registered connection to where your data lives. The query that feeds your widget reads through it. Pick one that's already registered, or register the buildings demo."
        >
          <DatasourceStep ws={ws} source={source} onPick={setSource} canRegister={canRunQuery} />
        </StepShell>
      ),
    },
    {
      key: "query",
      label: "Query",
      hint: "The rows to render",
      render: () => (
        <StepShell
          icon={ScrollText}
          title="Run the query your widget will render"
          blurb="This preloaded query summarises energy use per site over the last 4 days — total, peak, and each site's share of the leader. Run it: the rows it returns are exactly what your template renders, and what the AI prompt embeds as sample data."
        >
          <SqlPreviewStep source={source} canRun={canRunQuery} sql={sql} onRows={setRows} />
        </StepShell>
      ),
    },
    {
      key: "design",
      label: "Design",
      hint: "Write the widget",
      render: () => (
        <StepShell
          icon={Code2}
          title="Design & preview the widget"
          blurb="Pick a polished example — or “Draft with AI”: copy the prompt (it carries the engine rules + your real data), paste any agent's HTML into the editor, and preview it live here. The engine is pure {{…}} interpolation (no JavaScript): bind fields with {{site}}, iterate with {{#each rows}}…{{/each}}. Styling must be inline + host theme tokens (SVG is stripped — use CSS)."
        >
          <DesignStep
            ws={ws}
            source={source}
            sql={sql}
            code={code}
            onCode={setCode}
            rows={rows}
            example={example}
            onPickExample={pickExample}
          />
        </StepShell>
      ),
    },
    {
      key: "save",
      label: "Save",
      hint: "A reusable widget",
      render: () => (
        <StepShell
          icon={LayoutTemplate}
          title="Save the widget"
          blurb="Save it as a durable template — a reusable render_template you can drop into any panel from the Saved-template picker. Optionally, drop it straight onto a new dashboard so it's visible right away."
        >
          <SaveStep ws={ws} source={source} sql={sql} code={code} canSave={canSave} />
        </StepShell>
      ),
    },
  ];

  return <StepFlow steps={steps} finishLabel="Finish" onFinish={onDone} />;
}

// ── The intro list row — a numbered icon badge, the part name, and its one-liner. ──
function IntroItem({
  icon: Icon,
  n,
  title,
  body,
}: {
  icon: typeof Database;
  n: number;
  title: string;
  body: string;
}) {
  return (
    <li className="flex items-start gap-3 rounded-md border border-border bg-panel px-3 py-2.5">
      <span className="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary">
        <Icon size={15} />
      </span>
      <div className="min-w-0">
        <span className="block text-sm font-medium text-fg">
          <span className="text-muted">{n}.</span> {title}
        </span>
        <span className="mt-0.5 block text-xs text-muted">{body}</span>
      </div>
    </li>
  );
}

// ── Step 3: design ── a gallery of polished starter examples + the real inline template editor beside a
//    live WidgetHost preview of a `view:"template"` cell bound to the same query. Picking an example
//    seeds the code; editing the HTML re-renders the preview against the query's rows — the same
//    in-process TemplateView the dashboard uses (no fork, no mock).
function DesignStep({
  ws,
  source,
  sql,
  code,
  onCode,
  rows,
  example,
  onPickExample,
}: {
  ws: string;
  source: string;
  sql: string;
  code: string;
  onCode: (code: string) => void;
  rows: Row[];
  example: TemplateExample;
  onPickExample: (next: TemplateExample) => void;
}) {
  // The preview cell — rebuilt when the code or query changes so the WidgetHost re-renders the edited
  // template against the selected example's rows.
  const cell = useMemo(
    () => templateCell(ws, source, sql, code, "Template preview"),
    [ws, source, sql, code],
  );
  // TemplateSourceField edits a TemplateValue; the wizard only uses inline mode (a saved reference has
  // nothing to save yet). Map its onChange back to the raw code string.
  const value: TemplateValue = { mode: "inline", code };
  const onChange = (next: TemplateValue) => {
    if (next.mode === "inline") onCode(next.code);
  };
  // Whether the editor still holds the picked example's pristine code (so re-picking is a clean reset
  // and an edited example reads as "customised").
  const pristine = code === example.code;
  // The code editor is HIDDEN by default so the polished preview gets the whole width — a designed
  // widget should read as a widget, not a wall of JSX. The toggle reveals the editor to customise it.
  const [showCode, setShowCode] = useState(false);
  // "Draft with AI" opens the editor automatically — that's where the copied AI reply gets pasted.
  useEffect(() => {
    if (example.id === "ai") setShowCode(true);
  }, [example.id]);

  return (
    <div className="space-y-3">
      {/* The starter gallery — polished examples + a Draft-with-AI canvas; picking one seeds the editor. */}
      <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4" role="radiogroup" aria-label="starter templates">
        {TEMPLATE_GALLERY.map((ex) => {
          const active = ex.id === example.id;
          return (
            <Button
              key={ex.id}
              type="button"
              variant="ghost"
              role="radio"
              aria-checked={active}
              aria-label={`starter ${ex.id}`}
              onClick={() => onPickExample(ex)}
              className={`h-auto flex-col items-start gap-1 whitespace-normal rounded-lg border p-3 text-left ${
                active
                  ? "border-accent bg-accent/10 shadow-[inset_0_0_0_1px_hsl(var(--accent))]"
                  : "border-border hover:border-fg/30 hover:bg-fg/[0.02]"
              }`}
            >
              <span className="text-sm font-medium text-fg">{ex.label}</span>
              <span className="text-xs font-normal text-muted">{ex.description}</span>
            </Button>
          );
        })}
      </div>

      {/* The toolbar — Copy-AI-prompt (make one with any agent) + the show/hide-code toggle. The editor
          is collapsed by default so the widget preview is big; picking "Draft with AI" opens it. */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        {/* The prompt embeds the ran rows (via the provider) + this widget's query provenance, so an
            agent designs against the real shape. Copying it is the user's own data — their call. */}
        <ResultRowsProvider rows={rows}>
          <CopyTemplatePrompt query={{ tool: "federation.query", source, sql }} />
        </ResultRowsProvider>
        <Button
          type="button"
          variant="outline"
          size="sm"
          aria-pressed={showCode}
          aria-label={showCode ? "hide code" : "edit code"}
          className="h-7 gap-1.5"
          onClick={() => setShowCode((s) => !s)}
        >
          {showCode ? <EyeOff size={13} /> : <Code2 size={13} />}
          {showCode ? "Hide code" : "Edit code"}
        </Button>
      </div>

      <div className={showCode ? "grid gap-3 lg:grid-cols-2" : ""}>
        {showCode && (
          <div className="min-w-0">
            <TemplateSourceField value={value} onChange={onChange} />
            {!pristine && (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => onCode(example.code)}
                className="mt-1 h-auto px-1.5 py-0.5 text-[11px] text-muted hover:text-fg"
              >
                Reset to “{example.label}” starter
              </Button>
            )}
          </div>
        )}
        {/* Live preview — the exact in-process render path the dashboard grid uses. Full width + tall
            when the code is hidden so the widget has room to look good (these examples fill a real tile). */}
        <div
          className={`${showCode ? "h-[30rem]" : "h-[34rem]"} min-w-0 overflow-hidden rounded-lg border border-border bg-bg p-2 shadow-sm`}
          aria-label="widget preview"
        >
          <WidgetHost cell={cell} workspace={ws} />
        </div>
      </div>

      {rows.length === 0 && (
        <p className="text-xs text-muted">
          Tip: run the query in the previous step so the preview binds real rows (it renders empty until
          then).
        </p>
      )}
    </div>
  );
}

// ── Step 5: save ── the real `template.save`; optionally also drop the widget onto a fresh dashboard.
function SaveStep({
  ws,
  source,
  sql,
  code,
  canSave,
}: {
  ws: string;
  source: string;
  sql: string;
  code: string;
  canSave: boolean;
}) {
  const [title, setTitle] = useState("Energy by site");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedId, setSavedId] = useState<string | null>(null);
  const [dashboardId, setDashboardId] = useState<string | null>(null);

  const slug = () =>
    `render_template:${title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "widget"}`;

  const saveTpl = async () => {
    setBusy(true);
    setError(null);
    try {
      const id = savedId ?? slug();
      // engine "template" = the eval-free in-process view; the durable record stores the code only.
      const rec = await saveTemplate(id, title, "template", code);
      setSavedId(rec.id);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const addToDashboard = async () => {
    setBusy(true);
    setError(null);
    try {
      const id = `template-widget-${Date.now()}`;
      const cell = templateCell(ws, source, sql, code, title);
      await saveDashboard(id, title, [cell]);
      setDashboardId(id);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  if (!canSave) {
    return (
      <p className="text-xs text-muted">
        Saving a template needs widget-write access. Ask an admin, or copy the template HTML from the
        Design step and save it from a dashboard panel.
      </p>
    );
  }

  return (
    <div className="space-y-3">
      <label className="block space-y-1.5">
        <span className="text-xs font-medium text-muted">Template name</span>
        <Input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          aria-label="Template name"
          className="max-w-sm"
        />
      </label>

      <div className="flex flex-wrap items-center gap-2">
        <Button size="sm" disabled={busy || !title.trim()} onClick={() => void saveTpl()} aria-label="Save the template">
          {busy ? <Loader2 size={13} className="animate-spin" /> : <LayoutTemplate size={13} />}
          {savedId ? "Save again" : "Save template"}
        </Button>
        {savedId && (
          <span role="status" className="inline-flex items-center gap-1 text-xs text-accent">
            <Check size={13} /> Saved as <span className="font-mono">{savedId}</span>
          </span>
        )}
      </div>

      {savedId && (
        <div className="border-t border-border pt-3">
          {dashboardId ? (
            <div role="status" className="flex items-center gap-2 rounded-md border border-accent/25 bg-accent/10 px-3 py-2 text-xs text-accent">
              <Check size={14} /> Added to dashboard <span className="font-medium">{title}</span> — open it from the Dashboards page.
            </div>
          ) : (
            <Button variant="outline" size="sm" disabled={busy} onClick={() => void addToDashboard()} aria-label="Add to a new dashboard">
              <LayoutDashboard size={13} /> Add to a new dashboard
            </Button>
          )}
        </div>
      )}

      {error && (
        <p role="alert" className="text-xs text-red-500">
          {error}
        </p>
      )}
    </div>
  );
}
