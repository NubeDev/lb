// The param-declaration editor (rules-as-source: typed params, authoring loop) — the "Params" tab of
// the authoring panel. The author declares a rule's inputs: name, optional label, kind (text/number/
// date/enum), required, and (for enum) the allowed values. The declared list is co-owned with the body
// and persisted by `useRules` save/create; at run time the Data Studio params form renders one TYPED
// control per declaration and fills `rules.run`'s `args.params`, and `param("<name>")` reads it in the
// cage. One responsibility: edit the `RuleParam[]` list (no I/O — the parent hook persists).

import { Plus, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import type { ParamKind, RuleParam } from "@/lib/rules";

interface Props {
  params: RuleParam[];
  onChange: (next: RuleParam[]) => void;
}

const KINDS: ParamKind[] = ["text", "number", "date", "enum"];

export function ParamDeclEditor({ params, onChange }: Props) {
  const patchAt = (i: number, patch: Partial<RuleParam>) =>
    onChange(params.map((p, j) => (j === i ? { ...p, ...patch } : p)));
  const removeAt = (i: number) => onChange(params.filter((_, j) => j !== i));
  const add = () => onChange([...params, { name: "", kind: "text" }]);

  return (
    <div aria-label="params editor" className="flex min-h-0 flex-col">
      <div className="min-h-0 flex-1 overflow-auto p-3">
        {params.length === 0 ? (
          <p className="text-xs text-muted">
            No params. Declare an input the rule reads with <code>param(&quot;name&quot;)</code> — it
            becomes a typed field when the rule is used as a panel source.
          </p>
        ) : (
          <ul className="grid gap-3">
            {params.map((p, i) => {
              const kind = p.kind ?? "text";
              return (
                <li key={i} className="grid gap-1.5 rounded-md border border-border p-2">
                  <div className="flex items-center gap-1.5">
                    <Input
                      aria-label={`param name ${i}`}
                      className="h-7 flex-1 text-xs"
                      placeholder="name"
                      value={p.name}
                      onChange={(e) => patchAt(i, { name: e.target.value })}
                    />
                    <Button
                      aria-label={`remove param ${i}`}
                      size="sm"
                      variant="ghost"
                      onClick={() => removeAt(i)}
                    >
                      <Trash2 size={12} />
                    </Button>
                  </div>
                  <Input
                    aria-label={`param label ${i}`}
                    className="h-7 w-full text-xs"
                    placeholder="label (optional)"
                    value={p.label ?? ""}
                    onChange={(e) => patchAt(i, { label: e.target.value || undefined })}
                  />
                  <div className="flex items-center gap-1.5">
                    <Select
                      aria-label={`param kind ${i}`}
                      className="h-7 flex-1 text-xs"
                      value={kind}
                      onChange={(e) => patchAt(i, { kind: e.target.value as ParamKind })}
                    >
                      {KINDS.map((k) => (
                        <option key={k} value={k}>
                          {k}
                        </option>
                      ))}
                    </Select>
                    <label className="flex items-center gap-1 text-xs text-muted">
                      <input
                        aria-label={`param required ${i}`}
                        type="checkbox"
                        checked={p.required === true}
                        onChange={(e) => patchAt(i, { required: e.target.checked || undefined })}
                      />
                      required
                    </label>
                  </div>
                  {kind === "enum" && (
                    <Input
                      aria-label={`param options ${i}`}
                      className="h-7 w-full text-xs"
                      placeholder="options, comma-separated"
                      value={(p.options ?? []).join(", ")}
                      onChange={(e) =>
                        patchAt(i, {
                          options: e.target.value
                            .split(",")
                            .map((s) => s.trim())
                            .filter(Boolean),
                        })
                      }
                    />
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </div>
      <div className="shrink-0 border-t border-border p-2">
        <Button aria-label="add param" size="sm" variant="ghost" className="w-full" onClick={add}>
          <Plus size={12} /> Add param
        </Button>
      </div>
    </div>
  );
}
