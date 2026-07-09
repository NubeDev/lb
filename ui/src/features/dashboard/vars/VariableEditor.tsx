// The variable editor (widget-config-vars Slice 2) — add / edit / reorder dashboard variables in a
// settings drawer. Adding starts at the friendly TYPE PICKER (Grafana-parity); each variable is then a
// SECTIONED row (`VariableRow`) — collapsed to a summary line by default, expanded one at a time into
// General / Values / Selection / Advanced sections. Saving writes the DEFINITIONS to the record via
// `saveVariables` (the selection stays in the URL). Gated on the edit cap by the opener.

import { useState } from "react";
import { Plus } from "lucide-react";

import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { Button } from "@/components/ui/button";
import type { Variable, VariableType } from "@/lib/vars";
import { useSourcePicker } from "../builder/useSourcePicker";
import { VariableTypePicker } from "./VariableTypePicker";
import { VariableRow } from "./VariableRow";

/** The fixed resolver tool a `datasource` variable resolves against (advanced-variables scope). */
const DATASOURCE_TOOL = "datasource.list";

/** A fresh variable of `type` with the right empty value shape (the `datasource` type carries its fixed
 *  resolver so it resolves through the one `{tool,args}` path like any query variable). `name` is a
 *  deterministic default the author renames inline. */
function newVariable(type: VariableType, name: string): Variable {
  const base: Variable = { name, type };
  if (type === "custom") return { ...base, custom: [] };
  if (type === "interval") return { ...base, interval: [] };
  if (type === "datasource") return { ...base, query: { tool: DATASOURCE_TOOL } };
  return base;
}

/** A `varN` name not already taken by a draft variable (so two quick adds don't collide). */
function uniqueName(draft: Variable[]): string {
  const taken = new Set(draft.map((v) => v.name));
  let n = draft.length + 1;
  while (taken.has(`var${n}`)) n += 1;
  return `var${n}`;
}

interface Props {
  ws: string;
  variables: Variable[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSave: (variables: Variable[]) => void;
}

export function VariableEditor({ ws, variables, open, onOpenChange, onSave }: Props) {
  const { entries } = useSourcePicker(ws);
  const [draft, setDraft] = useState<Variable[]>(variables);
  // The "choose a type" step (Grafana-parity): shown for the first variable (empty state) and whenever
  // the author adds another. Picking a type creates the variable and returns to the list.
  const [picking, setPicking] = useState(false);
  // Which row is expanded (index) — one at a time, so the panel scans; -1 = all collapsed.
  const [expanded, setExpanded] = useState(0);

  // Re-seed the draft whenever the editor opens (so it reflects the current record, not a stale draft).
  const [wasOpen, setWasOpen] = useState(false);
  if (open && !wasOpen) {
    setDraft(variables);
    setPicking(false);
    setExpanded(variables.length ? 0 : -1);
    setWasOpen(true);
  }
  if (!open && wasOpen) setWasOpen(false);

  const update = (i: number, patch: Partial<Variable>) =>
    setDraft((d) => d.map((v, j) => (j === i ? { ...v, ...patch } : v)));
  /** Create a variable of the picked type (a unique default name), expand it, and leave the picker. */
  const addOfType = (type: VariableType) => {
    setDraft((d) => {
      setExpanded(d.length); // expand the newly appended row
      return [...d, newVariable(type, uniqueName(d))];
    });
    setPicking(false);
  };
  const remove = (i: number) => {
    setDraft((d) => d.filter((_, j) => j !== i));
    setExpanded((e) => (e === i ? -1 : e > i ? e - 1 : e));
  };
  const move = (i: number, dir: -1 | 1) =>
    setDraft((d) => {
      const j = i + dir;
      if (j < 0 || j >= d.length) return d;
      const next = [...d];
      [next[i], next[j]] = [next[j], next[i]];
      setExpanded((e) => (e === i ? j : e === j ? i : e));
      return next;
    });

  const save = () => {
    onSave(draft.filter((v) => v.name.trim()));
    onOpenChange(false);
  };

  const showPicker = picking || draft.length === 0;

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="w-full overflow-y-auto sm:max-w-lg" aria-label="variable editor">
        <SheetHeader>
          <SheetTitle>Dashboard variables</SheetTitle>
          <SheetDescription>Define variables once; reference them as $name across the dashboard.</SheetDescription>
        </SheetHeader>
        <div className="flex flex-col gap-3 px-4 pb-6">
          {showPicker ? (
            <VariableTypePicker
              onPick={addOfType}
              onCancel={picking && draft.length > 0 ? () => setPicking(false) : undefined}
            />
          ) : (
            <>
              <div className="flex flex-col gap-2">
                {draft.map((v, i) => (
                  <VariableRow
                    key={i}
                    variable={v}
                    entries={entries}
                    expanded={expanded === i}
                    onToggle={() => setExpanded((e) => (e === i ? -1 : i))}
                    onChange={(patch) => update(i, patch)}
                    onRemove={() => remove(i)}
                    onMoveUp={() => move(i, -1)}
                    onMoveDown={() => move(i, 1)}
                  />
                ))}
              </div>
              <div className="flex items-center gap-2 pt-1">
                <Button aria-label="add variable" size="sm" variant="outline" onClick={() => setPicking(true)}>
                  <Plus size={13} /> Add variable
                </Button>
                <Button aria-label="save variables" size="sm" className="ml-auto" onClick={save}>
                  Save variables
                </Button>
              </div>
            </>
          )}
        </div>
      </SheetContent>
    </Sheet>
  );
}
