// The "Edit workflow" dialog for the rules Workflows tab (rules-workflow-convergence scope). The
// one follow-up an author needs after creating a workflow: adjust WHEN it runs. Reuses the same
// trigger picker the Add dialog uses (`WorkflowTriggerPicker`) so the affordance reads identically
// in both directions. Submits via the hook's `updateTrigger`, which writes only the trigger node's
// config (`flows.node.update`) — the rule stays in place, the flow's other nodes are untouched.
//
// Scope is intentionally narrow: the rule + the workflow name are read-only context (the workflow's
// identity); the dialog edits the trigger. Renaming needs a full-flow re-save (there's no
// `flows.rename` verb), and re-pointing a workflow at a different rule is delete-and-recreate —
// both rare enough that crowding this dialog with them would obscure the common edit (tune the
// schedule). One responsibility per file (FILE-LAYOUT).

import { useEffect, useState, type FormEvent } from "react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { SavedRule } from "@/lib/rules";
import { WorkflowTriggerPicker } from "./WorkflowTriggerPicker";
import {
  defaultTriggerConfig,
  readTriggerConfig,
  type TriggerConfig,
} from "./workflowTrigger";

interface EditWorkflowDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The workflow being edited — supplies the name + the current trigger config. Null when closed. */
  workflow: {
    id: string;
    name: string;
    triggerConfig: Record<string, unknown> | null | undefined;
  } | null;
  /** The saved rule this workflow runs (read-only context — shown as "Running <rule>"). Null when the
   *  flow has no linked rule (a hand-built flow that landed in the tab). */
  rule: SavedRule | null;
  /** Submit delegates to the hook's `updateTrigger`; the dialog closes on a successful save. */
  onSave: (id: string, triggerConfig: TriggerConfig) => Promise<{ ok: boolean; error?: string }>;
}

export function EditWorkflowDialog({
  open,
  onOpenChange,
  workflow,
  rule,
  onSave,
}: EditWorkflowDialogProps) {
  const [config, setConfig] = useState<TriggerConfig>(defaultTriggerConfig());
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // When the dialog opens (or the target workflow changes), seed the picker from the workflow's
  // current trigger config. Don't re-seed on every render — only when the workflow id changes
  // (the parent's `workflow` object identity changes every render; `workflow?.id` is the real signal).
  useEffect(() => {
    if (open && workflow) {
      setConfig(readTriggerConfig(workflow.triggerConfig));
      setError(null);
      setSubmitting(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, workflow?.id]);

  function close() {
    if (submitting) return;
    onOpenChange(false);
  }

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    if (!workflow || submitting) return;
    setSubmitting(true);
    setError(null);
    const res = await onSave(workflow.id, config);
    if (res.ok) {
      onOpenChange(false);
    } else {
      setSubmitting(false);
      setError(res.error ?? "Couldn't save the trigger.");
    }
  }

  return (
    <Dialog open={open} onOpenChange={(o) => (o ? null : close())}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle>Edit workflow</DialogTitle>
          <DialogDescription>
            {workflow
              ? `Adjust when “${workflow.name}” runs.`
              : "Adjust when this workflow runs."}
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={onSubmit} className="flex flex-col gap-5">
          {/* Read-only context — the workflow's identity (what it runs). The dialog edits the trigger
              only; the rule + name are fixed here. */}
          <div className="flex items-center justify-between rounded-md border border-border bg-bg px-3 py-2 text-xs">
            <div className="flex flex-col">
              <span className="font-medium text-fg">{workflow?.name ?? "—"}</span>
              <span className="text-[10px] text-muted">
                {rule ? `Running ${rule.name}` : "No rule linked"}
              </span>
            </div>
          </div>

          <div className="flex flex-col gap-2">
            <span className="text-xs font-medium text-fg">When it runs</span>
            <WorkflowTriggerPicker config={config} onChange={setConfig} disabled={submitting} />
          </div>

          {error ? (
            <p role="alert" className="text-xs text-destructive">
              {error}
            </p>
          ) : null}

          <DialogFooter>
            <Button type="button" variant="ghost" onClick={close} disabled={submitting}>
              Cancel
            </Button>
            <Button type="submit" disabled={submitting}>
              {submitting ? "Saving…" : "Save changes"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
