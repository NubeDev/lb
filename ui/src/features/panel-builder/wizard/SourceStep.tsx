// SourceStep (panel-wizard scope) — the wizard's first step. Two tracks, both 100% reused surfaces:
//   - series/native: the SHIPPED source picker (`useSourcePicker` + `SourceCombobox`) the Query tab
//     mounts — the same ws-scoped entries, no second source surface;
//   - datasource (federation): pick a registered datasource and author against it through the FULL
//     `QueryWorkbench` — the exact page the Datasources detail mounts (Builder⇄Code, canvas, Run,
//     history, SAVED QUERIES). Running a query (or loading a saved SQL one) adopts it as the panel's
//     source via the workbench's `onUseSql` seam: the target becomes `federation.query {source, sql}`
//     (the same wire shape the editor's Query tab writes) — prove the SQL by running it.
//
// No wizard-only state: the chosen track lives entirely in `state.targets[0]` (+ `state.sql` for the
// authored query); the saved-query selection (`sel`) is presentation-only dialog state.
//
// One responsibility: pick a read source into the wizard's primary target.

import { useMemo, useState } from "react";
import { Lightbulb, ListTree, Database } from "lucide-react";

import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import type { Target, View } from "@/lib/dashboard";
import { canonicalView } from "@/lib/dashboard";
import { SOURCELESS_VIEWS } from "@/lib/panel-kit";
import { useSourcePicker } from "@/features/dashboard/builder/useSourcePicker";
import { READ_SOURCE_GROUPS, SourceCombobox, type SourceEntry, SQL_SOURCE_ID } from "@/features/dashboard/builder/sourcePicker";

/** The group order for the WORKSPACE-source combobox — Rules first (panel-wizard-source-discoverability
 *  scope: a user who clicked in on the word "rule" sees the "Rules" heading at a glance, not seventh).
 *  Derived from the canonical `READ_SOURCE_GROUPS` (same labels, one vocabulary — source-picker-package
 *  scope) with `flows` dropped (never a wizard source) and `rules` hoisted to the top. Reordering only —
 *  the wizard still routes every kind through the same generic `selectEntry` (CLAUDE §10). */
const WORKSPACE_SOURCE_GROUPS = (() => {
  const shown = READ_SOURCE_GROUPS.filter(({ group }) => group !== "flows");
  const rules = shown.filter(({ group }) => group === "rules");
  const rest = shown.filter(({ group }) => group !== "rules");
  return [...rules, ...rest];
})();
import { Select } from "@/components/ui/select";
import { defaultOptionsForView } from "@/features/panel-builder/viewOptions";
import { useDatasourceList, refForOption } from "@/features/panel-builder/tabs/useDatasourceList";
import { QueryWorkbench } from "@/features/query-workbench/QueryWorkbench";
import { RuleWorkbench } from "./RuleWorkbench";

interface Props {
  ws: string;
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
  /** Switch the panel's view (resets per-view options). The source step uses it for the sourceless
   *  views (e.g. Insights) — picking one clears the target so the "no source" gate is satisfied. */
  onPickView: (view: View) => void;
  /** Advance to the next wizard step. A complete source choice (picking Insights, or adopting a
   *  workspace source / datasource query) moves the flow forward without a second click on Next. */
  onAdvance: () => void;
}

/** The three source tracks the user chooses between up front — one card each, one selected at a time.
 *  The chosen track is DERIVED from state (no wizard-only field): Insights ⇐ a sourceless view,
 *  workspace ⇐ a non-federation target, datasource ⇐ a `federation.query` target. */
type Track = "insights" | "workspace" | "datasource";

/** The sourceless view the Insights track selects — it reads its own data, so it needs no target. Kept
 *  in sync with SOURCELESS_VIEWS (`insights` = the triage list, `@nube/insights`). */
const SOURCELESS_VIEW: View = "insights";

/** Build the primary target from a chosen picker entry (mirrors QueryTab.targetFromEntry). */
function targetFromEntry(entry: SourceEntry | null, prev: Target | undefined): Target {
  const refId = prev?.refId || "A";
  if (!entry || !entry.source) return { refId, tool: "", args: {}, datasource: { type: "surreal" } };
  const tool = entry.source.tool;
  return {
    refId,
    tool,
    args: (entry.source.args as Record<string, unknown>) ?? {},
    datasource: { type: tool === "store.query" ? "surreal" : "series" },
  };
}

export function SourceStep({ ws, state, patch, onPickView, onAdvance }: Props) {
  // EAGER: the wizard's whole purpose is to pick a source, so the picker loads on mount (unlike the
  // editor's Query tab, which waits for focus — the wizard IS the focused surface).
  const { entries, loading } = useSourcePicker(ws, { enabled: true });
  // Whether the panel is currently a SOURCELESS view (Insights) — it needs no target, so the source
  // pickers collapse and the sourceless card reads as selected.
  const currentView = canonicalView((state.view || "timeseries") as View);
  const isSourceless = SOURCELESS_VIEWS.has(currentView);
  const { options: dsOptions, loading: dsLoading } = useDatasourceList(ws);
  const fedOptions = dsOptions.filter((o) => o.type === "federation");
  const primary = state.targets[0];

  // The datasource track is DERIVED from the target (no second source of truth): a federation.query
  // target names its datasource in args.source.
  const fedSource = primary?.tool === "federation.query" ? ((primary.args?.source as string) ?? "") : "";
  const fedSql = primary?.tool === "federation.query" ? ((primary.args?.sql as string) ?? "") : "";
  // The saved-query dialog selection — presentation-only (the adopted SQL is what persists).
  const [sel, setSel] = useState<string | null>(null);

  // The active track — DERIVED from state so Back/Next never loses it, with a transient override for
  // the moment between clicking a track's card and picking within it (e.g. "datasource" chosen but no
  // federation target yet, so the workbench chooser can show). `null` = no manual pick this mount.
  const [pickedTrack, setPickedTrack] = useState<Track | null>(null);
  const derivedTrack: Track | null = isSourceless
    ? "insights"
    : fedSource || primary?.tool === "federation.query"
      ? "datasource"
      : primary?.tool
        ? "workspace"
        : null;
  const track = derivedTrack ?? pickedTrack;

  const TRACKS = useMemo(
    () =>
      [
        {
          id: "insights" as Track,
          label: "Insights",
          hint: "A triage list of findings from rules, flows & agents. No data source needed.",
          Icon: Lightbulb,
        },
        {
          id: "workspace" as Track,
          label: "Workspace source",
          hint: "Bind to a saved rule, series, or saved query already in this workspace.",
          Icon: ListTree,
        },
        ...(fedOptions.length > 0
          ? [
              {
                id: "datasource" as Track,
                label: "Datasource",
                hint: "Author a query against a registered datasource, with saved queries.",
                Icon: Database,
              },
            ]
          : []),
      ] as const,
    [fedOptions.length],
  );

  /** Choose a track's card. Insights is a complete choice → set the view and advance. Workspace and
   *  datasource reveal their chooser (a second pick binds the actual source). Re-picking the active
   *  track is a no-op beyond keeping it open. */
  const pickTrack = (next: Track) => {
    setPickedTrack(next);
    if (next === "insights") {
      pickSourceless(SOURCELESS_VIEW);
      onAdvance();
      return;
    }
    // Leaving Insights (or datasource↔workspace) — clear the prior track's binding so the revealed
    // chooser starts empty and the derived track follows the new pick.
    if (isSourceless) onPickView("timeseries");
    if (next === "workspace" && primary?.tool === "federation.query") selectDatasource("");
    if (next === "datasource" && primary?.tool && primary.tool !== "federation.query")
      patch({ sql: undefined, targets: [{ refId: primary?.refId || "A", tool: "", args: {}, datasource: { type: "surreal" } }] });
  };

  /** The picker entry matching the current target (so the combobox reopens showing the picked source,
   *  and the rule workbench reads the picked rule's declared `params`). Disambiguates a shared tool by
   *  its identifying arg: `series` for a series read, `rule_id` for a rule (every rule shares
   *  `rules.run`, so tool alone would collapse them — mirrors `sourcePicker.seedEntryId`). */
  const pickedEntry = primary?.tool
    ? entries.find((e) => {
        if (e.source?.tool !== primary.tool) return false;
        const pickedSeries = (primary.args as { series?: string } | undefined)?.series;
        if (pickedSeries) return (e.source.args as { series?: string } | undefined)?.series === pickedSeries;
        const pickedRuleId = (primary.args as { rule_id?: string } | undefined)?.rule_id;
        if (pickedRuleId) return (e.source.args as { rule_id?: string } | undefined)?.rule_id === pickedRuleId;
        return true;
      }) ?? null
    : null;
  const pickedEntryId = pickedEntry?.id ?? "";
  // A rule source shows the prove-it workbench (Run + params + rows) — the parity twin of the datasource
  // track's embedded QueryWorkbench. Derived from the target's tool (no wizard-only flag).
  const isRuleSource = primary?.tool === "rules.run";

  const selectEntry = (entry: SourceEntry | null) => {
    if (entry?.id === SQL_SOURCE_ID) {
      // The SQL source — empty SQL on first pick; the editor's Query tab is where the query is authored
      // (the wizard's simple track picks a labeled source). A user wanting SQL uses the editor.
      patch({
        sql: { mode: "code", rawSql: "", format: "table" },
        targets: [{ ...targetFromEntry(entry, primary), tool: "store.query", args: { sql: "" } }],
      });
      return;
    }
    patch({ sql: undefined, targets: [targetFromEntry(entry, primary)] });
  };

  /** Switch to the datasource track: a `federation.query` target with empty SQL — the embedded
   *  workbench authors it (mirrors QueryTab.selectDatasource's federation branch). */
  const selectDatasource = (name: string) => {
    setSel(null);
    if (!name) {
      patch({ sql: undefined, targets: [{ refId: primary?.refId || "A", tool: "", args: {}, datasource: { type: "surreal" } }] });
      return;
    }
    const opt = fedOptions.find((o) => o.name === name);
    const ds = opt ? refForOption(opt, ws) : { type: "federation" };
    patch({
      // SQL rows are arbitrary-shaped — default the preview to TABLE so the adopted query's rows show
      // immediately (a timeseries can't shape a summary result → an honest-but-unhelpful "no data
      // yet"). The chart-type step (step 2) is where the user picks the viz for chartable shapes.
      view: "table",
      options: defaultOptionsForView("table"),
      sql: { mode: "code", rawSql: "", format: "table" },
      targets: [{ refId: primary?.refId || "A", tool: "federation.query", args: { source: name, sql: "" }, datasource: ds }],
    });
  };

  /** Adopt the SQL the workbench just ran / loaded as the panel's source (the `onUseSql` seam). */
  const adoptSql = (sql: string) => {
    if (!fedSource) return;
    patch({
      sql: { mode: "code", rawSql: sql, format: "table" },
      targets: [{ ...(primary as Target), args: { source: fedSource, sql } }],
    });
  };

  /** Pick a sourceless view (Insights) — set the view (resets options to its defaults) and clear any
   *  target so the wizard's "no source" gate is satisfied. Picking the already-active one is a no-op. */
  const pickSourceless = (view: View) => {
    if (currentView === view) return;
    patch({ sql: undefined, targets: [{ refId: primary?.refId || "A", tool: "", args: {}, datasource: { type: "surreal" } }] });
    onPickView(view);
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4" aria-label="wizard source step">
      <div className="grid gap-1">
        <h2 className="text-sm font-medium text-fg">Where does this panel read from?</h2>
        <p className="text-xs text-muted">
          Pick one. Insights needs no data source; a workspace source or datasource binds real rows.
        </p>
      </div>

      {/* The three tracks — one selected at a time. Choosing a card either completes the step
          (Insights → advance) or reveals that track's chooser below. */}
      <div
        className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3"
        role="radiogroup"
        aria-label="source tracks"
      >
        {TRACKS.map(({ id, label, hint, Icon }) => {
          const selected = track === id;
          return (
            <button
              key={id}
              type="button"
              role="radio"
              aria-label={`source track ${id}`}
              aria-checked={selected}
              onClick={() => pickTrack(id)}
              className={`group flex flex-col gap-2 rounded-lg border p-3 text-left transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent ${
                selected
                  ? "border-accent bg-accent/10 shadow-[inset_0_0_0_1px_hsl(var(--accent))]"
                  : "border-border hover:border-fg/30 hover:bg-fg/[0.02]"
              }`}
            >
              <span
                className={`flex h-8 w-8 items-center justify-center rounded-md transition-colors ${
                  selected ? "bg-accent/15 text-accent" : "bg-fg/5 text-muted group-hover:text-fg"
                }`}
              >
                <Icon size={18} />
              </span>
              <span className="grid gap-0.5">
                <span className="text-sm font-medium text-fg">{label}</span>
                <span className="text-xs text-muted">{hint}</span>
              </span>
            </button>
          );
        })}
      </div>

      {/* The chosen track's chooser — only the selected track's controls, never all three at once. */}
      {track === "insights" && (
        <p className="text-xs text-muted" aria-label="wizard source picked">
          <code className="text-fg">insights</code> reads its own findings — no data source. Continue to{" "}
          <span className="text-fg">2. Chart type</span> to confirm.
        </p>
      )}

      {track === "workspace" && (
        <div className="grid gap-2">
          <label className="grid gap-1 text-xs text-muted">
            Source
            <SourceCombobox
              aria-label="wizard source"
              entries={entries}
              value={pickedEntryId}
              loading={loading}
              groups={WORKSPACE_SOURCE_GROUPS}
              onSelect={() => {}}
              onSelectEntry={(e) => selectEntry(e ?? null)}
            />
          </label>
          {/* The Rules group leads the combobox, but an empty group renders NO heading — so a workspace
              with no saved rules would leave the word the user came for invisible. Say it, and point at
              where rules are made, so the path is discoverable before the first rule exists. Deny-tolerant:
              a workspace without `mcp:rules.list:call` also yields no rules entries → same honest line. */}
          {!loading && !entries.some((e) => e.group === "rules") && (
            <p className="text-[11px] text-muted" aria-label="wizard no rules">
              No saved rules yet — create one in <span className="text-fg">Rules</span> to bind it here.
            </p>
          )}
          {/* A picked RULE gets the prove-it loop — params form + Run + rows — so it's tested before
              binding, at parity with the datasource track's embedded workbench (CLAUDE §10: this is the
              rule branch of the SAME generic source step, keyed on the target's tool, not a per-id case). */}
          {isRuleSource && (
            <RuleWorkbench
              target={primary as Target}
              params={pickedEntry?.params ?? []}
              onChange={(next) => patch({ targets: [next] })}
            />
          )}
        </div>
      )}

      {track === "datasource" && (
        <div className="grid min-h-0 flex-1 grid-rows-[auto_auto_1fr] gap-2">
          <label className="grid gap-1 text-xs text-muted">
            Datasource
            <Select
              aria-label="wizard datasource"
              className="h-8 w-full"
              value={fedSource}
              disabled={dsLoading}
              onChange={(e) => selectDatasource(e.target.value)}
            >
              <option value="">— choose a datasource —</option>
              {fedOptions.map((o) => (
                <option key={o.name} value={o.name}>
                  {o.label}
                </option>
              ))}
            </Select>
          </label>
          {fedSource ? (
            <>
              <p className="text-[11px] text-muted">
                Author against <code className="text-fg">{fedSource}</code> — Run a query (or load a
                saved one) to bind it as the panel's source.
              </p>
              <div className="min-h-[26rem] overflow-hidden rounded-md border border-border" aria-label="wizard datasource workbench">
                {/* `initial={state.sql}` — the wizard REMOUNTS this on Back/Next; seeding from the
                    persisted EditorState keeps the authored query instead of resetting the editor.
                    Fills the wizard's available height with a 26rem floor so it never collapses. */}
                <QueryWorkbench ws={ws} source={fedSource} sel={sel} onSel={setSel} onUseSql={adoptSql} initial={state.sql} />
              </div>
            </>
          ) : (
            <p className="text-[11px] text-muted">Pick a datasource to author its query.</p>
          )}
        </div>
      )}

      {/* The bound-source readout — shown for the query tracks (workspace / datasource) once a target
          exists, so the user sees exactly what will read before advancing. Insights has its own line. */}
      {track !== "insights" && primary?.tool && (
        <p className="text-[11px] text-muted" aria-label="wizard source picked">
          picked:{" "}
          <code className="text-fg">
            {primary.tool}
            {primary.args && primary.args.series ? ` → ${primary.args.series}` : ""}
            {fedSource ? ` → ${fedSource}${fedSql ? ` · ${fedSql.slice(0, 60)}${fedSql.length > 60 ? "…" : ""}` : " (run a query to bind it)"}` : ""}
          </code>
        </p>
      )}
    </div>
  );
}
