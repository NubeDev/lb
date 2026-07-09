// The "choose variable type" step (advanced-variables scope) — the friendly type picker shown when the
// author adds a new variable, before any row exists. A scannable list (icon tile + title + description),
// one row per `VariableType`, read from the shared catalog (`variableTypeMeta`). Picking a type creates
// the variable of that kind and returns to the list. FILE-LAYOUT: the picker is a view over the catalog.
//
// Not a card grid (design: cards are the lazy answer) — a dense, keyboard-navigable list that fits the
// product register: earned-familiar, scannable, no decoration. Each row is a real <button> so Enter/Space
// and focus rings come for free.

import { ArrowLeft } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { VariableType } from "@/lib/vars";
import { VARIABLE_TYPES } from "./variableTypeMeta";

export function VariableTypePicker({
  onPick,
  onCancel,
}: {
  onPick: (type: VariableType) => void;
  /** Return to the list without adding (only shown when there are existing variables to return to). */
  onCancel?: () => void;
}) {
  return (
    <div className="flex flex-col gap-3" aria-label="variable type picker">
      <div className="flex items-center gap-2">
        {onCancel && (
          <Button
            aria-label="cancel add variable"
            size="sm"
            variant="ghost"
            className="-ml-1.5 h-7 gap-1 px-1.5 text-muted"
            onClick={onCancel}
          >
            <ArrowLeft size={13} />
            Back
          </Button>
        )}
        <p className="text-xs font-medium text-muted">Choose a variable type</p>
      </div>

      <ul className="flex flex-col gap-1.5">
        {VARIABLE_TYPES.map(({ type, title, description, icon: Icon }) => (
          <li key={type}>
            {/* eslint-disable-next-line no-restricted-syntax -- a selectable type card (icon tile +
                title + description) is a custom clickable surface, not a shadcn Button use case */}
            <button
              type="button"
              aria-label={`add ${type} variable`}
              onClick={() => onPick(type)}
              className="group flex w-full items-start gap-3 rounded-lg border border-border bg-bg/40 p-3 text-left transition-colors hover:border-accent/40 hover:bg-accent/[0.06] focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/25"
            >
              <span className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-border bg-panel text-muted transition-colors group-hover:border-accent/30 group-hover:bg-accent/10 group-hover:text-accent">
                <Icon size={16} strokeWidth={1.75} />
              </span>
              <span className="min-w-0 flex-1">
                <span className="block text-[13px] font-medium text-fg">{title}</span>
                <span className="mt-0.5 block text-xs leading-relaxed text-muted">{description}</span>
              </span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
