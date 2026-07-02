// The horizontal stepper spine — the one element that always answers "where am I, what's left".
// Done steps show a check and are clickable to step back; the active step is accent-filled; upcoming
// steps sit quiet and disabled. A single connector line fills between nodes and colors as you progress.

import { Check } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { STUDIO_STEPS } from "./steps";
import type { StudioStep } from "./studio.wizard";

interface Props {
  step: StudioStep;
  onGoBack: (step: StudioStep) => void;
}

export function Stepper({ step, onGoBack }: Props) {
  return (
    <nav
      aria-label="Progress"
      className="mx-auto flex w-full max-w-2xl items-start px-1"
    >
      {STUDIO_STEPS.map((meta, i) => {
        const state =
          meta.n < step ? "done" : meta.n === step ? "active" : "upcoming";
        const Icon = meta.icon;
        const clickable = state === "done";
        return (
          <div key={meta.n} className="flex flex-1 items-start last:flex-none">
            <Button
              type="button"
              variant="ghost"
              disabled={!clickable}
              onClick={() => clickable && onGoBack(meta.n)}
              aria-current={state === "active" ? "step" : undefined}
              className={cn(
                "group h-auto min-w-0 flex-col gap-1.5 px-2 py-1 text-center hover:bg-transparent",
                "disabled:pointer-events-none disabled:opacity-100",
                clickable && "cursor-pointer",
                !clickable && "cursor-default",
              )}
            >
              <span
                className={cn(
                  "flex h-8 w-8 items-center justify-center rounded-full border text-xs font-semibold transition-colors",
                  state === "active" &&
                    "border-accent bg-accent text-bg shadow-sm shadow-accent/25",
                  state === "done" &&
                    "border-accent/40 bg-accent/10 text-accent group-hover:bg-accent/20",
                  state === "upcoming" && "border-border bg-bg text-muted/70",
                )}
              >
                {state === "done" ? (
                  <Check size={15} strokeWidth={2.5} />
                ) : (
                  <Icon size={15} />
                )}
              </span>
              <span className="flex flex-col">
                <span
                  className={cn(
                    "text-xs font-medium leading-tight",
                    state === "upcoming" ? "text-muted/70" : "text-fg",
                  )}
                >
                  {meta.label}
                </span>
                <span className="hidden text-[11px] leading-tight text-muted sm:block">
                  {meta.hint}
                </span>
              </span>
            </Button>
            {i < STUDIO_STEPS.length - 1 && (
              <span
                aria-hidden
                className={cn(
                  "mt-4 h-px flex-1 transition-colors",
                  meta.n < step ? "bg-accent/50" : "bg-border",
                )}
              />
            )}
          </div>
        );
      })}
    </nav>
  );
}
