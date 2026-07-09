// A reusable multi-step wizard frame (wizard scope). This is the GENERIC shell every guided flow in
// the app shares: the step rail (jump back to any reached step), a scrolling body that renders the
// active step, and a Back / Next footer. It owns ONLY navigation state (current step, how far you've
// reached) — a step's own data, verbs, and validation live in whatever it renders. That separation is
// what makes it reusable: the onboarding flow, the appearance flow, and any future wizard describe
// themselves as a `FlowStep[]` and hand it here; none re-implement stepping. One responsibility per
// file (FILE-LAYOUT).
//
// Reuse contract: pass `steps`, each with a `render(ctx)` that returns the step body. `ctx.next()` /
// `ctx.back()` let a step drive navigation from its own primary button (e.g. after a save succeeds).
// `canAdvance` optionally gates the footer Next per step. The frame is presentation + navigation; it
// stays out of the domain entirely (no caps, no gateway) so it drops into any feature.

import { useState } from "react";
import { ArrowLeft, ArrowRight, Check } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Stepper, type Step } from "../setup/Stepper";

export interface FlowContext {
  /** Zero-based index of the step currently rendering. */
  index: number;
  /** Advance to the next step (no-op on the last). */
  next: () => void;
  /** Go back one step (no-op on the first). */
  back: () => void;
  /** Whether this is the final step. */
  isLast: boolean;
}

export interface FlowStep extends Step {
  /** The step body. */
  render: (ctx: FlowContext) => React.ReactNode;
  /** Optionally block the footer Next until true (e.g. a required selection). Defaults to allowed. */
  canAdvance?: boolean;
}

interface Props {
  steps: FlowStep[];
  /** Label for the primary button on the last step (defaults to "Done"). */
  finishLabel?: string;
  /** Called when the last step's primary button is pressed. */
  onFinish?: () => void;
}

export function StepFlow({ steps, finishLabel = "Done", onFinish }: Props) {
  const [step, setStep] = useState(0);
  const [reached, setReached] = useState(0);

  const goto = (i: number) => {
    const clamped = Math.max(0, Math.min(steps.length - 1, i));
    setStep(clamped);
    setReached((r) => Math.max(r, clamped));
  };
  const ctx: FlowContext = {
    index: step,
    next: () => goto(step + 1),
    back: () => goto(step - 1),
    isLast: step === steps.length - 1,
  };

  const active = steps[step];
  const canAdvance = active?.canAdvance ?? true;

  return (
    <div className="flex h-full min-h-0 flex-col" data-testid="step-flow">
      <div className="border-b border-border bg-panel-2/40 px-4 py-3">
        <Stepper steps={steps} current={step} reached={reached} onJump={goto} />
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-5">
        <div className="mx-auto max-w-2xl">{active?.render(ctx)}</div>
      </div>

      <div className="flex items-center gap-3 border-t border-border bg-panel-2/40 px-4 py-3">
        <Button
          variant="outline"
          size="sm"
          disabled={step === 0}
          onClick={() => goto(step - 1)}
          aria-label="Back"
        >
          <ArrowLeft size={14} /> Back
        </Button>
        <div className="ml-auto">
          {ctx.isLast ? (
            <Button size="sm" onClick={onFinish} aria-label={finishLabel}>
              <Check size={14} /> {finishLabel}
            </Button>
          ) : (
            <Button
              size="sm"
              disabled={!canAdvance}
              onClick={() => goto(step + 1)}
              aria-label="Continue"
            >
              Continue <ArrowRight size={14} />
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}

// A consistent step frame — an icon-badged title, a one-line blurb, then the step's controls. Shared
// so every step across every wizard reads identically. Exported for reuse by any FlowStep.render.
export function StepShell({
  icon: Icon,
  title,
  blurb,
  children,
}: {
  icon: LucideIcon;
  title: string;
  blurb: string;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-4">
      <header className="flex items-start gap-3">
        <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
          <Icon size={18} />
        </span>
        <div className="min-w-0">
          <h3 className="text-base font-semibold text-fg">{title}</h3>
          <p className="mt-0.5 text-sm text-muted">{blurb}</p>
        </div>
      </header>
      {children}
    </section>
  );
}
