// The Data → insight wizard (setup scope) — one guided path from a raw datasource all the way to a
// live insight, one page per key part with a plain-language "what this is for" intro. It is PURE
// ORCHESTRATION over pieces that already exist:
//
//   1. Datasource — the real `datasource.list` / `datasource.add` verbs (`useDatasourceList` +
//      `addDatasource`), the same roster the Datasources page uses.
//   2. SQL        — the real `useQueryRun` dispatch + `QueryResults` grid (the Query workbench's
//      engine), with the query PRELOADED in a read-only `CodeEditor` (sql highlighting) + a Run button.
//   3. Panel      — a real v3 timeseries `Cell` (built by `timeseriesCell`, the panel wizard's prefill
//      shape) previewed live through the SAME `WidgetHost` the dashboard renders.
//   4. Dashboard  — `saveDashboard` (the real `dashboard.save` UPSERT) drops that panel into a fresh
//      dashboard; the effect is read-backable over the gateway.
//   5. Rule       — the canonical buildings rule PRELOADED read-only, RUN via the real `useRules`
//      (`setBuffer` + `run` → `runRule`), its output shown through the real `RunResult` pane.
//   6. Insights   — the real `InsightsReadWidget` (@nube/insights) over the shell `insightsClient`,
//      showing exactly what the rule raised, with a note on how deduped insights work.
//
// No new backend, no duplicated editor. The only new code is this flow + `dataToInsight.ts` (the three
// prefilled strings + one cell-builder). Cap-gating hides controls the caller couldn't use anyway; the
// gateway re-checks every write (rule 5). One responsibility per file (FILE-LAYOUT).

import { useMemo, useState } from "react";
import {
  BarChart3,
  Check,
  Database,
  Lightbulb,
  LayoutDashboard,
  Loader2,
  Play,
  ScrollText,
  Sparkles,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { CodeEditor, codeLanguageExtension } from "@/components/codeeditor";
import { CAP, hasCap } from "@/lib/session";
import { saveDashboard } from "@/lib/dashboard";
import { insightsClient } from "@/lib/insights/insights.client";
import { WidgetHost } from "@/features/dashboard/WidgetHost";
import { useRules } from "@/features/rules/useRules";
import { RunResult } from "@/features/rules/RunResult";
import { InsightsReadWidget } from "@nube/insights";
import { StepFlow, StepShell, type FlowStep } from "../wizard/StepFlow";
import { DatasourceStep } from "./steps/DatasourceStep";
import { SqlPreviewStep } from "./steps/SqlPreviewStep";
import { DEFAULT_SOURCE, DEMO_RULE, DEMO_SQL, timeseriesCell } from "./dataToInsight";

interface Props {
  ws: string;
  caps: string[] | undefined;
  onDone?: () => void;
}

// One CodeMirror language extension — resolved once (a stable array keeps CodeMirror from
// re-configuring on every render). The SQL step lives in the shared `SqlPreviewStep`.
const RHAI_EXT = [codeLanguageExtension("rhai")!];

export function DatasourceWizard({ ws, caps, onDone }: Props) {
  const canRunQuery = hasCap(caps, CAP.datasourceList); // reading a source ≈ the workbench nav gate
  const canSaveDash = hasCap(caps, CAP.dashboardSave);
  const canRunRule = hasCap(caps, CAP.rulesRun);
  const canSeeInsights = hasCap(caps, CAP.insightList);

  // Accumulated across steps: the chosen datasource, and the dashboard the panel landed in.
  const [source, setSource] = useState(DEFAULT_SOURCE);
  const [dashboardId, setDashboardId] = useState<string | null>(null);

  const steps: FlowStep[] = [
    {
      key: "intro",
      label: "Overview",
      hint: "The whole path",
      render: () => (
        <StepShell
          icon={Sparkles}
          title="From a datasource to a live insight"
          blurb="This walks the full data path end to end — connect a source, query it, chart it on a dashboard, then let a rule watch it and raise an insight. Each screen explains what that part is for and runs the real thing."
        >
          <ol className="space-y-2">
            <IntroItem icon={Database} n={1} title="Datasource — where the data lives" body="A registered connection (here a SQLite buildings dataset). Everything downstream reads through it." />
            <IntroItem icon={ScrollText} n={2} title="SQL — ask a question of it" body="Run a query and see rows back. This one averages energy per site, per hour." />
            <IntroItem icon={BarChart3} n={3} title="Panel — draw the answer" body="A timeseries chart, one line per site. The query's GROUP BY is what splits the lines." />
            <IntroItem icon={LayoutDashboard} n={4} title="Dashboard — save it somewhere" body="Drop that panel onto a fresh dashboard you (and your team) can return to." />
            <IntroItem icon={ScrollText} n={5} title="Rule — watch it automatically" body="A small script re-runs the query and raises an insight when a building goes over budget." />
            <IntroItem icon={Lightbulb} n={6} title="Insight — the durable finding" body="A deduped, acknowledgeable record of what the rule found. This is the payoff." />
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
          blurb="A datasource is a registered connection to where your data lives — a SQL database, a file, a series. The query, the panel, and the rule all read through it. Pick one that's already registered, or register the buildings demo."
        >
          <DatasourceStep
            ws={ws}
            source={source}
            onPick={setSource}
            canRegister={hasCap(caps, CAP.datasourceList)}
          />
        </StepShell>
      ),
    },
    {
      key: "sql",
      label: "SQL",
      hint: "Ask a question",
      render: () => (
        <StepShell
          icon={ScrollText}
          title="Run a SQL query"
          blurb="SQL is how you ask the datasource a question. This one is preloaded — it averages energy use per site, per hour, over the last 4 days. Press Run to see the rows it returns."
        >
          <SqlPreviewStep source={source} canRun={canRunQuery} />
        </StepShell>
      ),
    },
    {
      key: "panel",
      label: "Panel & dashboard",
      hint: "Draw + save it",
      render: () => (
        <StepShell
          icon={BarChart3}
          title="Chart it, then save it to a dashboard"
          blurb="A panel turns query rows into a picture. This is a timeseries — one line per site, because the query groups by site. Save it and it becomes a real dashboard you can reopen."
        >
          <PanelStep
            ws={ws}
            source={source}
            dashboardId={dashboardId}
            onSaved={setDashboardId}
            canSave={canSaveDash}
          />
        </StepShell>
      ),
    },
    {
      key: "rule",
      label: "Rule",
      hint: "Watch it",
      render: () => (
        <StepShell
          icon={ScrollText}
          title="Preview and run the rule"
          blurb="A rule is a small script that re-runs a query on a schedule and raises an insight when something crosses a threshold. This one ranks buildings by energy intensity and flags any over budget. Read it, then run it once here."
        >
          <RuleStep canRun={canRunRule} />
        </StepShell>
      ),
    },
    {
      key: "insights",
      label: "Insights",
      hint: "The payoff",
      render: () => (
        <StepShell
          icon={Lightbulb}
          title="See the insights it raised"
          blurb="An insight is a durable, acknowledgeable finding — deduped by key, so a rule that keeps firing bumps a count instead of spamming duplicates. Below are the insights in this workspace (the ones the rule just raised should appear here)."
        >
          <InsightsStep canSee={canSeeInsights} />
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

// ── Step 3+4: panel + dashboard ── build the real timeseries cell, PREVIEW it live through the same
//    WidgetHost the dashboard uses, then Save it into a fresh dashboard via the real `dashboard.save`.
function PanelStep({
  ws,
  source,
  dashboardId,
  onSaved,
  canSave,
}: {
  ws: string;
  source: string;
  dashboardId: string | null;
  onSaved: (id: string) => void;
  canSave: boolean;
}) {
  const cell = useMemo(() => timeseriesCell(ws, source, DEMO_SQL, "Energy per site"), [ws, source]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const id = `energy-by-site-${Date.now()}`;
      await saveDashboard(id, "Energy by site", [cell]);
      onSaved(id);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-3">
      {/* Live preview — the exact render path the dashboard grid uses (real bridge, not a mock). */}
      <div className="h-64 overflow-hidden rounded-md border border-border bg-panel p-2" aria-label="panel preview">
        <WidgetHost cell={cell} workspace={ws} />
      </div>

      {dashboardId ? (
        <div role="status" className="flex items-center gap-2 rounded-md border border-accent/25 bg-accent/10 px-3 py-2 text-xs text-accent">
          <Check size={14} />
          Saved to dashboard <span className="font-medium">Energy by site</span> — open it from the Dashboards page.
        </div>
      ) : canSave ? (
        <Button size="sm" disabled={busy} onClick={() => void save()} aria-label="Create the dashboard">
          <LayoutDashboard size={13} /> {busy ? "Saving…" : "Create dashboard with this panel"}
        </Button>
      ) : (
        <p className="text-xs text-muted">Saving a dashboard needs dashboard-write access. Ask an admin.</p>
      )}

      {error && (
        <p role="alert" className="text-xs text-red-500">
          {error}
        </p>
      )}
    </div>
  );
}

// ── Step 5: rule ── preloaded read-only, run through the real `useRules` (buffer + run). The result
//    pane is the workbench's own `RunResult`.
function RuleStep({ canRun }: { canRun: boolean }) {
  // The workbench hook owns the real run (buffer → `runRule`). The wizard PRELOADS the canonical rule
  // and sets it as the buffer at run time — the user runs it, doesn't author it here.
  const rules = useRules("setup");

  return (
    <div className="space-y-3">
      <div className="max-h-72 overflow-auto rounded-md border border-border">
        <CodeEditor value={DEMO_RULE} onChange={() => {}} extensions={RHAI_EXT} editable={false} ariaLabel="rule" height="auto" />
      </div>

      <div className="flex items-center gap-3">
        <Button
          size="sm"
          disabled={!canRun || rules.running}
          onClick={() => {
            rules.setBuffer(DEMO_RULE);
            void rules.run();
          }}
          aria-label="Run the rule"
        >
          {rules.running ? <Loader2 size={13} className="animate-spin" /> : <Play size={13} />}
          {rules.running ? "Running…" : "Run the rule"}
        </Button>
        {!canRun && <span className="text-xs text-muted">You don&apos;t have rule-run access here.</span>}
      </div>

      <RunResult result={rules.result} error={rules.error} running={rules.running} hasRun={rules.hasRun} view="table" />
    </div>
  );
}

// ── Step 6: insights ── the real read widget over the shell client. Shows what the rule raised.
function InsightsStep({ canSee }: { canSee: boolean }) {
  if (!canSee) {
    return <p className="text-xs text-muted">You don&apos;t have access to view insights in this workspace.</p>;
  }
  return (
    <div className="space-y-3">
      <div className="rounded-md border border-border bg-panel">
        <InsightsReadWidget client={insightsClient} title="Insights" filter={{ limit: 20 }} showRefresh />
      </div>
      <p className="text-xs text-muted">
        Each row is deduped by its <span className="font-mono">dedup_key</span> — re-running the rule
        bumps an existing insight&apos;s count rather than adding a duplicate. Acknowledge or resolve one
        from the Insights page.
      </p>
    </div>
  );
}
