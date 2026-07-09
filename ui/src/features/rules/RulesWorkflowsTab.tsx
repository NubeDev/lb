// The Workflows tab on the rules page (rules-workflow-convergence scope). This is the rule-author
// surface over the flows engine: every row is a saved flow that runs a rule on a trigger. The
// author picks a RULE + a TRIGGER (the "New workflow" dialog); the host stores it as a typed-node
// flow behind the scenes (`trigger → rule`). The user never sees "nodes" or "DAG" — they see a
// table of automations: the rule, the trigger, the enabled status, the next fire, and the actions
// (enable/disable, open in flow canvas, delete).
//
// Layout: a full-width, full-height shadcn Table with a slim toolbar above (count + the primary
// action). The table is the right affordance here — authors scan a column of similar automations
// the same way they scan a roster of reminders or members. No cards, no nested panels: rows on a
// surface, the same vocabulary as the rest of the admin surfaces (PeopleAdmin, ApiKeysAdmin).
//
// One responsibility per file (FILE-LAYOUT): this is the markup + the row; the data lives in
// `useRuleWorkflows`; the trigger presenters live in `workflowTrigger`; the create dialog in its own
// file. The rule roster comes from the parent (`useRules`) — one source of truth for the saved
// rules, shared with the editor.

import { useState } from "react";
import { ExternalLink, Pencil, Plus, Trash2, Workflow } from "lucide-react";

import { AppEmptyState } from "@/components/app/empty-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ConfirmDestructive } from "@/features/confirm/ConfirmDestructive";
import type { SavedRule } from "@/lib/rules";
import { AddWorkflowDialog } from "./AddWorkflowDialog";
import { EditWorkflowDialog } from "./EditWorkflowDialog";
import { useRuleWorkflows, type WorkflowRow } from "./useRuleWorkflows";
import { readTrigger } from "./workflowTrigger";

interface RulesWorkflowsTabProps {
  ws: string;
  /** The saved rules — used (a) to render the rule name on each row that has a linked rule, and
   *  (b) as the picker source in the Add dialog. Passed from the parent (already loaded). */
  ruleRoster: SavedRule[];
  /** Open a rule in the editor (jump back to the Editor tab with that rule loaded). */
  onJumpToRule?: (id: string) => void;
}

/** "in 12m" / "in 2h" / "in 3d" — a coarse relative read of the next cron fire. The host advances
 *  `nextAttemptTs` (a unix-sec instant) each tick; this is just a friendly slogan over that. */
function relativeNext(ts: number | undefined): string | null {
  if (!ts) return null;
  const now = Math.floor(Date.now() / 1000);
  const delta = ts - now;
  if (delta <= 0) return "due";
  if (delta < 60) return `in ${delta}s`;
  if (delta < 3600) return `in ${Math.round(delta / 60)}m`;
  if (delta < 86400) return `in ${Math.round(delta / 3600)}h`;
  return `in ${Math.round(delta / 86400)}d`;
}

/** The workflow name cell — a primary clickable affordance that opens the linked rule in the
 *  editor (the most common follow-up: "what does this run?"). Falls back to the flow's id when the
 *  flow has no name, and shows the rule id as a quiet caption when the rule isn't in the roster. */
function WorkflowNameCell({
  row,
  ruleName,
  onJumpToRule,
}: {
  row: WorkflowRow;
  ruleName: string | null;
  onJumpToRule?: (id: string) => void;
}) {
  const canOpenRule = row.ruleId !== null && onJumpToRule;
  return (
    <TableCell>
      {/* Mirror the Trigger cell's two-line rhythm exactly (`leading-tight`, no gap, `text-xs`
          primary + `text-[10px]` caption) so both stacks are the same height — otherwise this
          taller stack skews the row's vertical center and the single-line Status/Actions cells
          read as "not lined up". */}
      <div className="flex flex-col leading-tight">
        {canOpenRule ? (
          <Button
            variant="ghost"
            className="h-auto justify-start p-0 text-xs font-medium text-fg underline-offset-2 hover:bg-transparent hover:underline"
            onClick={() => row.ruleId && onJumpToRule(row.ruleId)}
          >
            {row.name || row.id}
          </Button>
        ) : (
          <span className="text-xs font-medium text-fg">{row.name || row.id}</span>
        )}
        {row.ruleId ? (
          <span className="font-mono text-[10px] text-muted">
            {ruleName ? ruleName : row.ruleId}
          </span>
        ) : (
          <span className="text-[10px] text-muted">no rule linked</span>
        )}
      </div>
    </TableCell>
  );
}

function WorkflowListRow({
  row,
  ws,
  ruleName,
  onJumpToRule,
  onToggle,
  onDelete,
  onEdit,
}: {
  row: WorkflowRow;
  ws: string;
  ruleName: string | null;
  onJumpToRule?: (id: string) => void;
  onToggle: (id: string, enabled: boolean) => void;
  onDelete: (id: string) => void;
  /** Open the Edit dialog for this row's trigger. */
  onEdit: (row: WorkflowRow) => void;
}) {
  const [confirming, setConfirming] = useState(false);
  const triggerNode = row.trigger;
  const triggerView = triggerNode
    ? readTrigger(triggerNode.config)
    : null;
  const TriggerIcon = triggerView?.icon;
  const next = row.enabled && triggerView?.mode === "cron" ? relativeNext(row.nextAttemptTs) : null;

  return (
    <TableRow>
      <WorkflowNameCell row={row} ruleName={ruleName} onJumpToRule={onJumpToRule} />

      <TableCell>
        {triggerView && TriggerIcon ? (
          <div className="flex items-center gap-2">
            <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md border border-border bg-bg text-muted">
              <TriggerIcon size={13} />
            </span>
            <div className="flex flex-col leading-tight">
              <span className="text-xs font-medium text-fg">{triggerView.label}</span>
              {triggerView.caption ? (
                <span className="text-[10px] text-muted">{triggerView.caption}</span>
              ) : null}
            </div>
          </div>
        ) : (
          <span className="text-xs text-muted">—</span>
        )}
      </TableCell>

      <TableCell>
        <div className="flex items-center gap-2">
          <Badge variant={row.enabled ? "success" : "secondary"}>
            {row.enabled ? "Enabled" : "Disabled"}
          </Badge>
          {next ? <span className="text-[10px] text-muted">Next {next}</span> : null}
        </div>
      </TableCell>

      <TableCell className="text-right">
        <div className="flex items-center justify-end gap-1">
          <Switch
            aria-label={row.enabled ? `disable ${row.name || row.id}` : `enable ${row.name || row.id}`}
            checked={row.enabled}
            onCheckedChange={(checked) => onToggle(row.id, checked)}
          />
          <Button
            aria-label={`edit workflow ${row.name || row.id}`}
            variant="ghost"
            size="icon"
            title="Edit trigger"
            onClick={() => onEdit(row)}
          >
            <Pencil size={14} />
          </Button>
          <Button
            asChild
            variant="ghost"
            size="icon"
            aria-label={`open ${row.name || row.id} in flow canvas`}
            title="Open in flow canvas"
          >
            <a href={`#/t/${encodeURIComponent(ws)}/flows/${encodeURIComponent(row.id)}`}>
              <ExternalLink size={15} />
            </a>
          </Button>
          <Button
            aria-label={`delete workflow ${row.name || row.id}`}
            variant="ghost"
            size="icon"
            className="text-muted hover:text-destructive"
            onClick={() => setConfirming(true)}
          >
            <Trash2 size={14} />
          </Button>
        </div>
      </TableCell>

      {confirming ? (
        <ConfirmDestructive
          title={`Delete ${row.name || row.id}`}
          consequence="This workflow and its saved graph will be removed. It can be recreated but its current configuration is not recoverable."
          reversible={false}
          escalation="none"
          confirmLabel="Delete"
          onConfirm={() => {
            onDelete(row.id);
            setConfirming(false);
          }}
          onCancel={() => setConfirming(false)}
        />
      ) : null}
    </TableRow>
  );
}

export function RulesWorkflowsTab({ ws, ruleRoster, onJumpToRule }: RulesWorkflowsTabProps) {
  const state = useRuleWorkflows(ws);
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<WorkflowRow | null>(null);

  const ruleNameById = (id: string | null) =>
    id ? (ruleRoster.find((r) => r.id === id)?.name ?? null) : null;
  const ruleById = (id: string | null): SavedRule | null =>
    id ? (ruleRoster.find((r) => r.id === id) ?? null) : null;

  return (
    <div className="flex h-full min-h-0 flex-col">
      {/* Toolbar — the row-level affordance sits in each row; this owns the page-wide count + the
          primary action. Stretches the full width (the table fills the rest). */}
      <div className="flex items-center justify-between gap-2 border-b border-border bg-panel/60 px-4 py-2.5">
        <div className="flex items-baseline gap-2">
          <h2 className="text-sm font-semibold text-fg">Workflows</h2>
          <span className="text-xs text-muted">
            {state.rows.length === 0
              ? "none yet"
              : `${state.rows.length} workflow${state.rows.length === 1 ? "" : "s"}`}
          </span>
        </div>
        <Button size="sm" onClick={() => setAdding(true)}>
          <Plus size={14} /> New workflow
        </Button>
      </div>

      {state.error ? (
        <div
          role="alert"
          className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-sm text-destructive"
        >
          {state.error}
        </div>
      ) : null}

      {/* The table region owns its own scroll — the rows can be many, the header stays put. */}
      <div className="min-h-0 flex-1 overflow-auto">
        {state.rows.length === 0 && !state.loading ? (
          <AppEmptyState
            icon={Workflow}
            title="No workflows yet"
            description="Add a workflow to pick a rule and when it runs. The automation is wired for you behind the scenes."
          />
        ) : (
          <Table>
            <TableHeader className="sticky top-0 z-10">
              <TableRow className="hover:bg-transparent">
                <TableHead className="w-[40%]">Workflow</TableHead>
                <TableHead className="w-[25%]">Trigger</TableHead>
                <TableHead className="w-[20%]">Status</TableHead>
                <TableHead className="w-[15%] text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {state.rows.map((row) => (
                <WorkflowListRow
                  key={row.id}
                  row={row}
                  ws={ws}
                  ruleName={ruleNameById(row.ruleId)}
                  onJumpToRule={onJumpToRule}
                  onToggle={state.toggle}
                  onDelete={state.remove}
                  onEdit={setEditing}
                />
              ))}
            </TableBody>
          </Table>
        )}
      </div>

      <AddWorkflowDialog
        open={adding}
        onOpenChange={setAdding}
        ruleRoster={ruleRoster}
        onCreate={async (input) => {
          const res = await state.create(input);
          return { ok: res.ok, error: res.error };
        }}
      />

      <EditWorkflowDialog
        open={editing !== null}
        onOpenChange={(o) => !o && setEditing(null)}
        workflow={
          editing
            ? {
                id: editing.id,
                name: editing.name || editing.id,
                triggerConfig: editing.trigger?.config ?? null,
              }
            : null
        }
        rule={editing ? ruleById(editing.ruleId) : null}
        onSave={async (id, triggerConfig) => state.updateTrigger(id, triggerConfig)}
      />
    </div>
  );
}
