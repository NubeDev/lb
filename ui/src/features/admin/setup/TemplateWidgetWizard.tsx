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
// No new backend, no new verb, no duplicated editor. New code is this flow + `templateCell`/
// `TEMPLATE_STARTER` in `dataToInsight.ts`. Cap-gating hides controls the caller couldn't use anyway;
// the gateway re-checks every write (rule 5). Datasource/source ids stay opaque (rule 10). One
// responsibility per file (FILE-LAYOUT).

import { useMemo, useState } from "react";
import {
  Check,
  Code2,
  Database,
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
import { DEFAULT_SOURCE, DEMO_SQL, TEMPLATE_STARTER, templateCell } from "./dataToInsight";

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

  // Accumulated across steps: the chosen datasource, the ran rows (feed preview + prompt), the template
  // HTML the user is designing, and where a saved template/dashboard landed.
  const [source, setSource] = useState(DEFAULT_SOURCE);
  const [rows, setRows] = useState<Row[]>([]);
  const [code, setCode] = useState(TEMPLATE_STARTER);

  const steps: FlowStep[] = [
    {
      key: "intro",
      label: "Overview",
      hint: "The whole path",
      render: () => (
        <StepShell
          icon={Sparkles}
          title="Build a render-template widget"
          blurb="A render-template widget turns query rows into a custom HTML panel — no charting library, just your own markup bound to the data. This walks you from the seeded buildings datasource, through the query, to designing the widget (and asking an AI to draft it), then saves it as a reusable template."
        >
          <ol className="space-y-2">
            <IntroItem icon={Database} n={1} title="Datasource — where the data lives" body="A registered connection (here the SQLite buildings dataset). The query reads through it." />
            <IntroItem icon={ScrollText} n={2} title="Query — the rows to render" body="Run a preloaded query and see the rows. Your widget will render exactly this shape." />
            <IntroItem icon={Code2} n={3} title="Design — write the widget" body="Edit the template HTML/JSX beside a live preview, bound to your real query rows." />
            <IntroItem icon={Sparkles} n={4} title="Ask an AI — draft it for you" body="Copy a prompt with the engine rules + your real data; paste any agent's reply back into the editor." />
            <IntroItem icon={LayoutTemplate} n={5} title="Save — a reusable widget" body="Persist it as a durable template you can drop into any panel, and optionally onto a new dashboard." />
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
          blurb="This preloaded query averages energy use per site, per hour, over the last 4 days. Run it — the rows it returns are exactly what your template will render, and what the AI prompt embeds as sample data."
        >
          <SqlPreviewStep source={source} canRun={canRunQuery} sql={DEMO_SQL} onRows={setRows} />
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
          title="Design the widget"
          blurb="Edit the template HTML on the left; the live preview on the right renders it against your real query rows. The engine is pure {{…}} interpolation (no JavaScript) — bind fields with {{site}}, iterate with {{#each rows}}…{{/each}}. Styling must be inline + host theme tokens."
        >
          <DesignStep ws={ws} source={source} code={code} onCode={setCode} rows={rows} />
        </StepShell>
      ),
    },
    {
      key: "ai",
      label: "Ask an AI",
      hint: "Draft it for you",
      render: () => (
        <StepShell
          icon={Sparkles}
          title="Let an AI draft the widget"
          blurb="Copy a prompt that includes the template-engine rules AND a sample of your real rows, then paste it into any LLM. Paste its reply back into the Design step's editor. The data in the prompt is your own on-screen query result — sharing it is your call."
        >
          {/* The prompt embeds the ran rows (via the provider) + this widget's query provenance. */}
          <ResultRowsProvider rows={rows}>
            <div className="space-y-3">
              <CopyTemplatePrompt query={{ tool: "federation.query", source, sql: DEMO_SQL }} />
              <p className="text-xs text-muted">
                {rows.length === 0
                  ? "Run the query in the previous step first so the prompt carries real sample rows."
                  : `The prompt will embed a sample of the ${rows.length} rows your query returned.`}
              </p>
            </div>
          </ResultRowsProvider>
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
          <SaveStep ws={ws} source={source} code={code} canSave={canSave} />
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

// ── Step 3: design ── the real inline template editor beside a live WidgetHost preview of a
//    `view:"template"` cell bound to the same query. Editing the HTML re-renders the preview against
//    the query's rows — the same in-process TemplateView the dashboard uses (no fork, no mock).
function DesignStep({
  ws,
  source,
  code,
  onCode,
  rows,
}: {
  ws: string;
  source: string;
  code: string;
  onCode: (code: string) => void;
  rows: Row[];
}) {
  // The preview cell — rebuilt when the code changes so the WidgetHost re-renders the edited template.
  const cell = useMemo(
    () => templateCell(ws, source, DEMO_SQL, code, "Template preview"),
    [ws, source, code],
  );
  // TemplateSourceField edits a TemplateValue; the wizard only uses inline mode (a saved reference has
  // nothing to save yet). Map its onChange back to the raw code string.
  const value: TemplateValue = { mode: "inline", code };
  const onChange = (next: TemplateValue) => {
    if (next.mode === "inline") onCode(next.code);
  };

  return (
    <div className="grid gap-3 lg:grid-cols-2">
      <div className="min-w-0">
        <TemplateSourceField value={value} onChange={onChange} />
      </div>
      {/* Live preview — the exact in-process render path the dashboard grid uses. */}
      <div className="h-72 min-w-0 overflow-hidden rounded-md border border-border bg-panel p-2" aria-label="widget preview">
        <WidgetHost cell={cell} workspace={ws} />
      </div>
      {rows.length === 0 && (
        <p className="text-xs text-muted lg:col-span-2">
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
  code,
  canSave,
}: {
  ws: string;
  source: string;
  code: string;
  canSave: boolean;
}) {
  const [title, setTitle] = useState("Hourly energy by site");
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
      const cell = templateCell(ws, source, DEMO_SQL, code, title);
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
