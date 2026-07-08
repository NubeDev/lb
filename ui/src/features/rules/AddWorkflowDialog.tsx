// The "New workflow" dialog for the rules Workflows tab (rules-workflow-convergence scope). This is
// the rule-author surface over the flows engine: the user picks a RULE + a TRIGGER, and the host
// stores it as a typed-node flow (`trigger → rule`). The user never sees "nodes", "graphs", or
// "DAG" — only the friendly contract: "run <this rule> when <this trigger fires>".
//
// The dialog writes the SAME trigger config the descriptor's schema validates (`mode` + mode-specific
// fields, mirroring `TriggerConfigFields`), and the SAME `rule` node config the `rule` node reads
// (`config.rule = "<rule_id>"`), so ajv validity holds at `flows.save` time — this is a presenter
// over the same shapes, not a parallel one. On submit, the hook's `create` builds the flow behind the
// scenes; the user sees only a row in the table.

import { useMemo, useState, type FormEvent } from "react";
import { Check, Search } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import type { SavedRule } from "@/lib/rules";
import { WorkflowTriggerPicker } from "./WorkflowTriggerPicker";
import type { CreateWorkflowInput } from "./useRuleWorkflows";
import { defaultTriggerConfig, type TriggerConfig } from "./workflowTrigger";

interface AddWorkflowDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The saved rules — the picker lists these. Passed from the parent (already loaded by useRules). */
  ruleRoster: SavedRule[];
  onCreate: (input: CreateWorkflowInput) => Promise<{ ok: boolean; error?: string }>;
}

export function AddWorkflowDialog({ open, onOpenChange, ruleRoster, onCreate }: AddWorkflowDialogProps) {
  const [ruleId, setRuleId] = useState<string | null>(null);
  const [ruleQuery, setRuleQuery] = useState("");
  const [name, setName] = useState("");
  const [trigger, setTrigger] = useState<TriggerConfig>(defaultTriggerConfig());
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const filteredRules = useMemo(() => {
    const q = ruleQuery.trim().toLowerCase();
    const sorted = [...ruleRoster].sort((a, b) => a.name.localeCompare(b.name));
    if (!q) return sorted;
    return sorted.filter((r) => r.name.toLowerCase().includes(q) || r.id.toLowerCase().includes(q));
  }, [ruleRoster, ruleQuery]);

  function reset() {
    setRuleId(null);
    setRuleQuery("");
    setName("");
    setTrigger(defaultTriggerConfig());
    setSubmitting(false);
    setError(null);
  }

  function close() {
    if (submitting) return;
    reset();
    onOpenChange(false);
  }

  function pickRule(r: SavedRule) {
    setRuleId(r.id);
    // Pre-fill the workflow name the first time a rule is picked — most workflows share the rule's
    // name; the author can still rename. Don't clobber an edited name on a re-pick.
    if (!name.trim()) setName(r.name);
  }

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    if (submitting) return;
    if (!ruleId) {
      setError("Pick a rule to run.");
      return;
    }
    const trimmedName = name.trim();
    if (!trimmedName) {
      setError("Give the workflow a name.");
      return;
    }

    setSubmitting(true);
    setError(null);
    const res = await onCreate({
      name: trimmedName,
      ruleId,
      triggerConfig: trigger,
      enabled: true,
    });
    if (res.ok) {
      reset();
      onOpenChange(false);
    } else {
      setSubmitting(false);
      setError(res.error ?? "Couldn't create the workflow.");
    }
  }

  const pickedRule = ruleRoster.find((r) => r.id === ruleId) ?? null;

  return (
    <Dialog open={open} onOpenChange={(o) => (o ? null : close())}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle>New workflow</DialogTitle>
          <DialogDescription>
            Pick a rule and when it should run. The automation is wired for you — open it in the flow canvas to refine.
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={onSubmit} className="flex flex-col gap-5">
          {/* Rule picker — the whole point of this surface. A small searchable list, single-select. */}
          <fieldset className="flex flex-col gap-2">
            <legend className="text-xs font-medium text-fg">Rule to run</legend>
            <div className="relative">
              <Search size={13} className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-muted" />
              <Input
                aria-label="search rules"
                placeholder="Search rules…"
                value={ruleQuery}
                onChange={(e) => setRuleQuery(e.target.value)}
                className="h-8 pl-7"
              />
            </div>
            <div className="max-h-44 overflow-auto rounded-md border border-border bg-bg">
              {filteredRules.length === 0 ? (
                <div className="px-3 py-3 text-xs text-muted">
                  {ruleRoster.length === 0 ? "No saved rules. Save one in the Editor first." : "No rules match."}
                </div>
              ) : (
                <ul role="listbox" aria-label="rules" className="flex flex-col">
                  {filteredRules.map((r) => {
                    const active = ruleId === r.id;
                    return (
                      <li key={r.id} role="option" aria-selected={active}>
                        <Button
                          type="button"
                          variant="ghost"
                          onClick={() => pickRule(r)}
                          className={cn(
                            "flex h-auto w-full items-center gap-2 rounded-none px-3 py-2 text-left text-xs",
                            active ? "bg-accent/10 text-accent hover:bg-accent/10" : "text-fg",
                          )}
                        >
                          <span className="min-w-0 flex-1 truncate font-medium">{r.name}</span>
                          <span className="truncate font-mono text-[10px] text-muted">{r.id}</span>
                          {active ? <Check size={13} className="shrink-0" /> : null}
                        </Button>
                      </li>
                    );
                  })}
                </ul>
              )}
            </div>
            {pickedRule ? (
              <p className="text-[11px] text-muted">
                Runs <span className="text-fg">{pickedRule.name}</span>
              </p>
            ) : null}
          </fieldset>

          {/* Name — the row's identity. Pre-filled from the rule name; the author can rename. */}
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="workflow-name">Workflow name</Label>
            <Input
              id="workflow-name"
              placeholder="e.g. Nightly review"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>

          {/* Trigger picker — WHEN the rule runs. Shared with the Edit dialog. */}
          <div className="flex flex-col gap-2">
            <span className="text-xs font-medium text-fg">When it runs</span>
            <WorkflowTriggerPicker config={trigger} onChange={setTrigger} />
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
            <Button type="submit" disabled={submitting || !ruleId || !name.trim()}>
              {submitting ? "Creating…" : "Create workflow"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
