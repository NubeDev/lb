// The dashboard surface (dashboard scope) — the roster + the selected dashboard's live grid of
// widgets over real series. Layout edits (add/remove/drag/resize a cell) persist through
// `dashboard.save` (the SurrealDB record, rule 4); widgets read `series.read`/`series.latest` and go
// live over the series SSE (motion, rule 3). Wiring + layout only; each piece owns its data.

import { useEffect, useState } from "react";
import { LayoutGrid, Share2, Variable as VariableIcon } from "lucide-react";

import { AppPage } from "@/components/app/page";
import { AppEmptyState } from "@/components/app/empty-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { DashboardRoster } from "./DashboardRoster";
import { Grid } from "./Grid";
import { AddLibraryPanel } from "./AddLibraryPanel";
import { VariableBar } from "./vars/VariableBar";
import { VariableEditor } from "./vars/VariableEditor";
import { RequiredVarGate, unboundRequiredVars } from "./vars/RequiredVarGate";
import { useVarScope } from "./vars/useVarScope";
import { ConfirmDestructive } from "@/features/confirm/ConfirmDestructive";
import { RefreshControl } from "./RefreshControl";
import { useAutoRefresh } from "./useAutoRefresh";
import { useDashboard } from "./useDashboard";
import { useSourcePicker } from "./builder/useSourcePicker";
import { DashboardCacheProvider } from "./cache/DashboardQueryProvider";
import type { Cell, Variable, Visibility } from "@/lib/dashboard";
import type { DashboardSearch } from "@/features/routing/search";
import { varsFromSearch, withVar } from "@/features/routing/search";
import { useAppRoutingContext } from "@/features/routing/RoutingContextProvider";
import { CAP, hasCap } from "@/lib/session";

const VISIBILITIES: Visibility[] = ["private", "team", "workspace"];

interface Props {
  ws: string;
  range?: DashboardSearch;
  /** Update the whole dashboard search (range, refresh, var-* selection) — one router navigate.
   *  Variable selection + refresh ride here so they round-trip in the URL (Slices 2/4). */
  onSearchChange?: (search: DashboardSearch) => void;
}

/** The dashboard surface, wrapped in its per-visit read cache. `DashboardCacheProvider` is keyed on `ws`
 *  so a workspace switch remounts it (fresh query client + fresh ws-prefixed keys — no cross-ws bleed),
 *  and leaving the route unmounts it (the cache clears, the scope's "clear on leave"). */
export function DashboardView(props: Props) {
  return (
    <DashboardCacheProvider key={props.ws} ws={props.ws}>
      <DashboardViewInner {...props} />
    </DashboardCacheProvider>
  );
}

function DashboardViewInner({ ws, range, onSearchChange }: Props) {
  const dash = useDashboard(ws);
  const picker = useSourcePicker(ws);
  const current = dash.current;
  // The selected dashboard id lives in the URL (`?d=<id>`) so a pasted/copied link re-opens the same
  // dashboard. Load the URL's dashboard whenever the id changes and doesn't match what's loaded; write
  // the id back to the URL on a roster select so the link is always honest + shareable.
  const selectedId = range?.d;
  useEffect(() => {
    if (selectedId && selectedId !== current?.id) void dash.select(selectedId);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId, ws]);
  const selectDashboard = (id: string) => {
    if (range) onSearchChange?.({ ...range, d: id });
    else void dash.select(id);
  };
  const removeDashboard = (id: string) => {
    void dash.remove(id);
    // Drop the URL id when the removed dashboard was the selected one, so the link never dangles at a
    // deleted id (falls back to the roster).
    if (range && range.d === id) onSearchChange?.({ ...range, d: undefined });
  };
  const [varEditorOpen, setVarEditorOpen] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  // The edit cap, sourced from the session grant the shell already holds (the same `caps` the nav gates
  // editing surfaces on) — no new backend read. The host re-checks `dashboard.save` regardless.
  const { caps } = useAppRoutingContext();
  const canEdit = hasCap(caps, CAP.dashboardSave);
  // The current variable selection (from the URL) + a writer that updates one variable's URL param.
  const selectedVars = range ? varsFromSearch(range) : {};
  const setVar = (name: string, value: string | string[] | undefined) => {
    if (range) onSearchChange?.(withVar(range, name, value));
  };
  // Resolve the variable scope shell-side (Slice 3): the URL selection + the built-ins from the token +
  // time range. Interpolated into every cell call + handed to extension tiles as ctx.vars/ctx.timeRange.
  const scope = useVarScope(current?.variables ?? [], range, current?.id ?? "", ws);
  // reusable-pages: a template dashboard's `required` variables that are still UNBOUND (no URL value,
  // no default). While any is unbound the grid must NOT fire (a `$site`-literal query is a footgun) —
  // we render the honest `RequiredVarGate` in place of the grid, before any cell bridge call.
  const unboundRequired = unboundRequiredVars(current?.variables ?? [], scope);
  // Auto-refresh (Slice 4): the URL interval drives a tick that re-resolves query variables + re-runs
  // each read cell's source (the watch streams compose — motion vs state). Pauses when the tab is hidden.
  const refreshKey = useAutoRefresh(range?.refresh);
  const copyLink = () => {
    if (typeof navigator !== "undefined" && navigator.clipboard) {
      void navigator.clipboard.writeText(window.location.href);
    }
  };

  return (
    <AppPage
      label="dashboard view"
      icon={LayoutGrid}
      title={current?.title ?? "Dashboards"}
      description="Live workspace dashboards and series widgets."
      workspace={ws}
      error={dash.error}
      actions={
        <>
          {current && (
            <Badge variant="outline" className="rounded-full uppercase">
              {current.visibility}
            </Badge>
          )}
          {current && (current.variables ?? []).some((v) => v.required) && (
            // reusable-pages: this dashboard is a TEMPLATE — surface its parameter count (a small hint,
            // not a new record type; a template is just a dashboard with required variables).
            <Badge variant="outline" className="rounded-full" title="A template page — a required variable must be picked to load it.">
              template · {(current.variables ?? []).filter((v) => v.required).length} param
              {(current.variables ?? []).filter((v) => v.required).length === 1 ? "" : "s"}
            </Badge>
          )}
          {range && (
            <div className="hidden items-center gap-1 text-xs text-muted md:flex">
              <Input
                aria-label="dashboard range from"
                className="h-8 w-[8.5rem] text-xs"
                type="date"
                value={range.from}
                onChange={(e) => onSearchChange?.({ ...range, from: e.target.value })}
              />
              <span>to</span>
              <Input
                aria-label="dashboard range to"
                className="h-8 w-[8.5rem] text-xs"
                type="date"
                value={range.to}
                onChange={(e) => onSearchChange?.({ ...range, to: e.target.value })}
              />
            </div>
          )}
          {range && current && (
            <RefreshControl
              value={range.refresh}
              onChange={(refresh) => onSearchChange?.({ ...range, refresh })}
            />
          )}
          {current && (
            <>
              <Button
                aria-label="copy dashboard link"
                variant="ghost"
                size="icon"
                className="h-8 w-8"
                title="Copy link"
                onClick={copyLink}
              >
                <Share2 size={13} />
              </Button>
              <Select
                aria-label="dashboard visibility"
                className="h-8 w-auto"
                value={current.visibility}
                onChange={(e) => void dash.share(e.target.value as Visibility)}
              >
                {VISIBILITIES.map((v) => (
                  <option key={v} value={v}>
                    {v}
                  </option>
                ))}
              </Select>
              {canEdit && (
                <Button
                  aria-label="edit variables"
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8"
                  title="Dashboard variables"
                  onClick={() => setVarEditorOpen(true)}
                >
                  <VariableIcon size={13} />
                </Button>
              )}
              <Button
                aria-label="delete dashboard"
                variant="destructive"
                size="sm"
                onClick={() => setConfirmDelete(true)}
              >
                Delete
              </Button>
            </>
          )}
        </>
      }
    >
      <DashboardRoster
        roster={dash.roster}
        selectedId={current?.id ?? null}
        onSelect={selectDashboard}
        onCreate={dash.create}
        onRename={(id, title) => void dash.rename(id, title)}
        onRemove={removeDashboard}
        canEdit={canEdit}
      />

      {current && confirmDelete && (
        <ConfirmDestructive
          title={`Delete ${current.title}`}
          consequence="This dashboard and its widget layout will be removed. It can be recreated but its current layout is not recoverable."
          reversible={false}
          escalation="none"
          confirmLabel="Delete"
          onConfirm={() => {
            removeDashboard(current.id);
            setConfirmDelete(false);
          }}
          onCancel={() => setConfirmDelete(false)}
        />
      )}

      {!current ? (
        <AppEmptyState
          icon={LayoutGrid}
          title="Select or create a dashboard."
          description="Dashboards stay scoped to the current workspace and can be shared when needed."
        />
      ) : (
        <div className="flex min-w-0 flex-1 flex-col">
          <VariableBar
            variables={current.variables ?? []}
            selected={selectedVars}
            onChange={setVar}
            refreshKey={refreshKey}
          />
          <VariableEditor
            ws={ws}
            variables={current.variables ?? []}
            open={varEditorOpen}
            onOpenChange={setVarEditorOpen}
            onSave={(vars: Variable[]) => void dash.saveVariables(vars)}
          />
          {/* Panel AUTHORING lives in Data Studio now (data-studio scope v2) — the dashboard only
              PLACES library panels (ref cells) and renders. Build/edit panels at /t/$ws/data-studio. */}
          {canEdit && (
            <div className="flex items-center gap-2 border-b border-border bg-panel px-3 py-2">
              <AddLibraryPanel
                existing={current.cells}
                onAdd={(cell: Cell) => void dash.saveCells([...current.cells, cell])}
              />
            </div>
          )}

          <div className="min-h-0 flex-1">
            {unboundRequired.length > 0 ? (
              // A template with an unbound page parameter — gate the grid (no cell fires until picked).
              <RequiredVarGate unbound={unboundRequired} />
            ) : (
              <Grid
                cells={current.cells}
                editable
                range={range}
                scope={scope}
                refreshKey={refreshKey}
                installed={picker.installed}
                workspace={ws}
                onLayout={(cells) => void dash.saveCells(cells)}
                onRemove={(i) => void dash.saveCells(current.cells.filter((c) => c.i !== i))}
              />
            )}
          </div>
        </div>
      )}
    </AppPage>
  );
}
