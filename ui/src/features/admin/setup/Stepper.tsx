// The wizard step rail (setup scope) — the horizontal progress indicator across the top of the Setup
// wizard. Purely presentational: it shows the four steps, marks done/current/upcoming, and lets you
// jump BACK to any completed step (never forward past an incomplete one). Tokens only — no colour
// literals; the accent is the shell's `primary`. One responsibility per file (FILE-LAYOUT).

import { Check } from "lucide-react";

import { cn } from "@/lib/utils";

export interface Step {
  key: string;
  label: string;
  hint: string;
}

interface Props {
  steps: Step[];
  /** Zero-based index of the current step. */
  current: number;
  /** How far the user has progressed (indices < this are "done" and jumpable). */
  reached: number;
  onJump: (index: number) => void;
}

export function Stepper({ steps, current, reached, onJump }: Props) {
  return (
    <ol className="flex items-stretch gap-1" aria-label="Setup progress">
      {steps.map((step, i) => {
        const done = i < reached;
        const active = i === current;
        const jumpable = i <= reached;
        return (
          <li key={step.key} className="flex flex-1 items-center">
            {/* eslint-disable-next-line no-restricted-syntax -- a step-rail cell, not a shadcn Button shape */}
            <button
              type="button"
              disabled={!jumpable}
              onClick={() => jumpable && onJump(i)}
              aria-current={active ? "step" : undefined}
              className={cn(
                "group flex w-full items-center gap-3 rounded-lg border px-3 py-2.5 text-left transition-colors",
                active
                  ? "border-primary/60 bg-primary/10"
                  : done
                    ? "border-border bg-panel hover:border-primary/40"
                    : "border-border/60 bg-panel/40",
                jumpable ? "cursor-pointer" : "cursor-default",
              )}
            >
              {/* The numbered/checked chip — filled when done, ringed when active, muted upcoming. */}
              <span
                className={cn(
                  "flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-xs font-semibold transition-colors",
                  done
                    ? "bg-primary text-primary-foreground"
                    : active
                      ? "border-2 border-primary text-primary"
                      : "border border-border text-muted",
                )}
              >
                {done ? <Check size={14} strokeWidth={3} /> : i + 1}
              </span>
              <span className="min-w-0">
                <span
                  className={cn(
                    "block truncate text-sm font-medium",
                    active || done ? "text-fg" : "text-muted",
                  )}
                >
                  {step.label}
                </span>
                <span className="block truncate text-xs text-muted">{step.hint}</span>
              </span>
            </button>
          </li>
        );
      })}
    </ol>
  );
}
