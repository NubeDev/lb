// The flow runtime-state banner — the answer to "is this flow running?" when you open it. A cron/
// source flow's runs are each finite, so there's usually no live run to point at; what the user needs
// to see is that the flow is ARMED (firing headless on a schedule), when it fires next, and that runs
// are accumulating (the count "going up"). A manual flow shows it's idle (runs on demand). One
// component, presentational only — all state is derived in `armedState.ts`. (FILE-LAYOUT.)

import { Radio, Clock, Pause, Power, Square } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

import type { FlowArmedState } from "./armedState";
import { relativeFuture, relativePast } from "./armedState";

export interface FlowArmedBannerProps {
  armed: FlowArmedState;
  /** Injected 1s clock so "next fire in N" / "last fired N ago" tick live (and tests are stable). */
  nowSecs: number;
  /** How many runs the flow has (the "count going up" — shown for an armed flow). */
  runCount: number;
  /** Flip the durable enabled flag (`flows.enable`): Deploy (arm) when disabled, Stop (disarm) when
   *  armed. This is the headless-flow Stop — durable, so it survives a restart. Omit to hide it. */
  onToggle?: () => void;
}

export function FlowArmedBanner({ armed, nowSecs, runCount, onToggle }: FlowArmedBannerProps) {
  if (armed.kind === "idle") {
    // A manual flow needs no armed banner — Run drives it on demand; the v-pinned banner covers a run.
    return null;
  }

  const disabled = armed.kind === "disabled";
  const lastFired = armed.latestRun?.ts;

  return (
    <div
      aria-label="flow armed state"
      data-armed={armed.kind}
      className={cn(
        "flex items-center gap-2 px-3 py-1.5 text-xs",
        disabled
          ? "bg-muted/40 text-muted-foreground"
          : "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300",
      )}
    >
      {disabled ? (
        <Pause className="size-3.5 shrink-0" aria-hidden />
      ) : (
        <Radio className="size-3.5 shrink-0 animate-pulse" aria-hidden />
      )}
      <span className="font-medium">
        {disabled ? "Disabled — nothing fires." : "Armed — running headless."}
      </span>
      {!disabled && armed.cron ? (
        <span className="text-emerald-700/80 dark:text-emerald-300/80">
          schedule <code className="font-mono">{armed.cron}</code>
        </span>
      ) : null}
      {!disabled && armed.nextFireTs ? (
        <span className="flex items-center gap-1" aria-label="next fire">
          <Clock className="size-3" aria-hidden />
          next fire {relativeFuture(armed.nextFireTs, nowSecs)}
        </span>
      ) : null}
      <span className="ml-auto flex items-center gap-3 text-emerald-700/80 dark:text-emerald-300/80">
        <span aria-label="run count">
          {runCount} run{runCount === 1 ? "" : "s"}
        </span>
        {lastFired != null ? (
          <span aria-label="last fired">last fired {relativePast(lastFired, nowSecs)}</span>
        ) : (
          <span aria-label="last fired">no runs yet</span>
        )}
        {onToggle ? (
          <Button
            aria-label={disabled ? "deploy flow" : "stop flow"}
            onClick={onToggle}
            size="sm"
            variant={disabled ? "default" : "destructive"}
            className="h-6 gap-1.5 px-2"
          >
            {disabled ? <Power size={12} aria-hidden /> : <Square size={12} aria-hidden />}
            {disabled ? "Deploy" : "Stop"}
          </Button>
        ) : null}
      </span>
    </div>
  );
}
