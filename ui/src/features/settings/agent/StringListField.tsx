// A small add/remove list editor for a `string[]` axis (agent-personas scope #1) — the granted-tools
// and grounding-skills lists in the persona editor. A text input + Add button appends; each entry has a
// remove button. Values are OPAQUE strings (rule 10) — this primitive treats every entry as plain data,
// never validating or branching on a specific id. Presentation + local state only.

import { useState, type KeyboardEvent } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Field } from "../Field";

interface Props {
  label: string;
  /** The stable aria-label prefix ("granted tools") for the input + each chip's remove button. */
  ariaLabel: string;
  placeholder?: string;
  values: string[];
  onChange: (next: string[]) => void;
}

export function StringListField({ label, ariaLabel, placeholder, values, onChange }: Props) {
  const [draft, setDraft] = useState("");

  const add = () => {
    const v = draft.trim();
    if (!v || values.includes(v)) {
      setDraft("");
      return;
    }
    onChange([...values, v]);
    setDraft("");
  };

  const remove = (v: string) => onChange(values.filter((x) => x !== v));

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      add();
    }
  };

  return (
    <Field label={label}>
      <div className="flex flex-col gap-2">
        <div className="flex items-center gap-2">
          <Input
            aria-label={`${ariaLabel} input`}
            value={draft}
            placeholder={placeholder}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={onKeyDown}
          />
          <Button
            size="sm"
            variant="outline"
            onClick={add}
            aria-label={`add ${ariaLabel}`}
            disabled={!draft.trim()}
          >
            Add
          </Button>
        </div>
        {values.length > 0 && (
          <ul className="flex flex-wrap gap-1.5" aria-label={`${ariaLabel} list`}>
            {values.map((v) => (
              <li
                key={v}
                className="flex items-center gap-1 rounded-md bg-panel px-2 py-0.5 text-xs text-fg"
              >
                <span className="font-mono">{v}</span>
                <Button
                  variant="ghost"
                  size="sm"
                  aria-label={`remove ${ariaLabel} ${v}`}
                  className="h-4 w-4 p-0 text-muted hover:text-red-500"
                  onClick={() => remove(v)}
                >
                  ×
                </Button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </Field>
  );
}
