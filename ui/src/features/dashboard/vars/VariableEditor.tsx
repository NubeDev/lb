// The variable editor (widget-config-vars Slice 2) — add / edit / reorder dashboard variables in a
// settings drawer. Each variable has a type, name, label, and a type-specific value source: a `query`/
// `source` variable picks its resolver via the source picker (the same friendly-label picker the widget
// builder uses — the author never types a tool name); `custom`/`interval` carry a comma list; `text`/
// `const` a single value. Saving writes the DEFINITIONS to the record via `saveVariables` (the selection
// stays in the URL). Gated on the edit cap by the opener.

import { useState } from "react";
import { Plus, Trash2, ArrowUp, ArrowDown } from "lucide-react";

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
import type { SourceEntry } from "../builder/sourcePicker";

const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

const TYPES: VariableType[] = ["query", "custom", "text", "const", "interval", "source"];

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

  // Re-seed the draft whenever the editor opens (so it reflects the current record, not a stale draft).
  const [wasOpen, setWasOpen] = useState(false);
  if (open && !wasOpen) {
    setDraft(variables);
    setWasOpen(true);
  }
  if (!open && wasOpen) setWasOpen(false);

  const update = (i: number, patch: Partial<Variable>) =>
    setDraft((d) => d.map((v, j) => (j === i ? { ...v, ...patch } : v)));
  const add = () =>
    setDraft((d) => [...d, { name: `var${d.length + 1}`, type: "custom", custom: [] }]);
  const remove = (i: number) => setDraft((d) => d.filter((_, j) => j !== i));
  const move = (i: number, dir: -1 | 1) =>
    setDraft((d) => {
      const j = i + dir;
      if (j < 0 || j >= d.length) return d;
      const next = [...d];
      [next[i], next[j]] = [next[j], next[i]];
      return next;
    });

  const save = () => {
    onSave(draft.filter((v) => v.name.trim()));
    onOpenChange(false);
  };

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="w-full overflow-y-auto sm:max-w-lg" aria-label="variable editor">
        <SheetHeader>
          <SheetTitle>Dashboard variables</SheetTitle>
          <SheetDescription>Define variables once; reference them as $name across the dashboard.</SheetDescription>
        </SheetHeader>
        <div className="flex flex-col gap-3 px-4 pb-4 text-xs">
          {draft.map((v, i) => (
            <VariableRow
              key={i}
              variable={v}
              entries={entries}
              onChange={(patch) => update(i, patch)}
              onRemove={() => remove(i)}
              onMoveUp={() => move(i, -1)}
              onMoveDown={() => move(i, 1)}
            />
          ))}
          <div className="flex items-center gap-2">
            <Button aria-label="add variable" size="sm" variant="outline" onClick={add}>
              <Plus size={12} /> Add variable
            </Button>
            <Button aria-label="save variables" size="sm" className="ml-auto" onClick={save}>
              Save variables
            </Button>
          </div>
        </div>
      </SheetContent>
    </Sheet>
  );
}

// A comma-list input that keeps the raw typed text locally (so a mid-typed comma isn't eaten by a
// live split/re-join), emitting the parsed list on every change. Re-seeds when the values change shape.
function ListField({
  label,
  placeholder,
  values,
  onChange,
}: {
  label: string;
  placeholder: string;
  values: string[];
  onChange: (values: string[]) => void;
}) {
  const [text, setText] = useState(values.join(", "));
  return (
    /* eslint-disable-next-line no-restricted-syntax -- token-bound native input */
    <input
      aria-label={label}
      className={`${FIELD} w-64`}
      placeholder={placeholder}
      value={text}
      onChange={(e) => {
        setText(e.target.value);
        onChange(e.target.value.split(",").map((s) => s.trim()).filter(Boolean));
      }}
    />
  );
}

function VariableRow({
  variable,
  entries,
  onChange,
  onRemove,
  onMoveUp,
  onMoveDown,
}: {
  variable: Variable;
  entries: SourceEntry[];
  onChange: (patch: Partial<Variable>) => void;
  onRemove: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
}) {
  const isQuery = variable.type === "query" || variable.type === "source";
  // The current query entry id (match the picker entry whose source.tool/args equals the variable's).
  const selectedEntryId =
    entries.find(
      (e) => e.source?.tool === variable.query?.tool && (variable.type === "source" ? e.group === "extension" || e.group === "series" : true),
    )?.id ?? "";

  return (
    <div className="rounded-md border border-border bg-bg/50 p-2" aria-label={`variable row ${variable.name}`}>
      <div className="flex flex-wrap items-center gap-2">
        {/* eslint-disable-next-line no-restricted-syntax -- token-bound native input; no shadcn Input variant */}
        <input
          aria-label="variable name"
          className={`${FIELD} w-28`}
          placeholder="name"
          value={variable.name}
          onChange={(e) => onChange({ name: e.target.value.replace(/[^\w.]/g, "") })}
        />
        {/* eslint-disable-next-line no-restricted-syntax -- token-bound native input */}
        <input
          aria-label="variable label"
          className={`${FIELD} w-28`}
          placeholder="label"
          value={variable.label ?? ""}
          onChange={(e) => onChange({ label: e.target.value })}
        />
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive; token-bound */}
        <select
          aria-label="variable type"
          className={FIELD}
          value={variable.type}
          onChange={(e) => onChange({ type: e.target.value as VariableType })}
        >
          {TYPES.map((t) => (
            <option key={t} value={t}>
              {t}
            </option>
          ))}
        </select>
        <button aria-label="move variable up" className="icon-button" onClick={onMoveUp}>
          <ArrowUp size={12} />
        </button>
        <button aria-label="move variable down" className="icon-button" onClick={onMoveDown}>
          <ArrowDown size={12} />
        </button>
        <button
          aria-label="remove variable"
          className="rounded-md p-1 text-muted hover:bg-red-500/10 hover:text-red-500"
          onClick={onRemove}
        >
          <Trash2 size={12} />
        </button>
      </div>

      <div className="mt-2 flex flex-wrap items-center gap-2">
        {isQuery && (
          /* eslint-disable-next-line no-restricted-syntax -- token-bound native select; the friendly source picker */
          <select
            aria-label="variable query source"
            className={`${FIELD} w-64`}
            value={selectedEntryId}
            onChange={(e) => {
              const entry = entries.find((x) => x.id === e.target.value);
              if (entry?.source) onChange({ query: { tool: entry.source.tool, args: entry.source.args } });
            }}
          >
            <option value="">— pick a source —</option>
            {entries
              .filter((e) => e.source && !e.writes)
              .map((e) => (
                <option key={e.id} value={e.id}>
                  {e.label}
                </option>
              ))}
          </select>
        )}
        {variable.type === "custom" && (
          <ListField
            label="variable custom values"
            placeholder="prod, staging, dev (comma-separated)"
            values={variable.custom ?? []}
            onChange={(custom) => onChange({ custom })}
          />
        )}
        {variable.type === "interval" && (
          <ListField
            label="variable interval values"
            placeholder="1m, 5m, 1h (comma-separated)"
            values={variable.interval ?? []}
            onChange={(interval) => onChange({ interval })}
          />
        )}
        {variable.type === "text" && (
          /* eslint-disable-next-line no-restricted-syntax -- token-bound native input */
          <input
            aria-label="variable text default"
            className={`${FIELD} w-64`}
            placeholder="default text"
            value={variable.text ?? ""}
            onChange={(e) => onChange({ text: e.target.value })}
          />
        )}
        {variable.type === "const" && (
          /* eslint-disable-next-line no-restricted-syntax -- token-bound native input */
          <input
            aria-label="variable const value"
            className={`${FIELD} w-64`}
            placeholder="fixed value"
            value={variable.const ?? ""}
            onChange={(e) => onChange({ const: e.target.value })}
          />
        )}
        {isQuery && (
          <>
            <label className="flex items-center gap-1 text-muted">
              <input
                aria-label="variable multi"
                type="checkbox"
                checked={!!variable.multi}
                onChange={(e) => onChange({ multi: e.target.checked })}
              />
              multi
            </label>
            <label className="flex items-center gap-1 text-muted">
              <input
                aria-label="variable include all"
                type="checkbox"
                checked={!!variable.includeAll}
                onChange={(e) => onChange({ includeAll: e.target.checked })}
              />
              include all
            </label>
          </>
        )}
      </div>
    </div>
  );
}
