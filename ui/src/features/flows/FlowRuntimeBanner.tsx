// The flow runtime-state banner — the PLC answer to "is this flow running?" when you open it. A flow
// is a long-lived runtime: ENABLED → running (its reactors advance node values every tick, whether or
// not any single finite run is in flight), DISABLED → stopped. That is the whole model; there is no
// "armed vs idle" (a flow with no self-firing source still shows "running" — it just advances only
// when you Run it). One component, presentational only — all state is derived in `runtimeState.ts`.
// (FILE-LAYOUT.)

import { Radio, Clock, Pause } from "lucide-react";

import { cn } from "@/lib/utils";

import type { FlowRuntimeState } from "./runtimeState";
import { relativeFuture, relativePast } from "./runtimeState";

export interface FlowRuntimeBannerProps {
  runtime: FlowRuntimeState;
  /** Injected 1s clock so "next fire in N" / "last fired N ago" tick live (and tests are stable). */
  nowSecs: number;
  /** How many runs the flow has (the "count going up"). */
  runCount: number;
}

// Purely INFORMATIONAL (flow-deploy-ux scope): running vs stopped, the schedule (if any), next fire,
// run count. Enable/Disable lives in the toolbar (`FlowToolbar`) so there is one place to control the
// flow — the banner never owns a toggle.
export function FlowRuntimeBanner({ runtime, nowSecs, runCount }: FlowRuntimeBannerProps) {
  const stopped = runtime.state === "stopped";
  const lastFired = runtime.latestRun?.ts;

  return (
    <div
      aria-label="flow runtime state"
      data-state={runtime.state}
      className={cn(
        "flex items-center gap-2 px-3 py-1.5 text-xs",
        stopped
          ? "bg-muted/40 text-muted-foreground"
          : "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300",
      )}
    >
      {stopped ? (
        <Pause className="size-3.5 shrink-0" aria-hidden />
      ) : (
        <Radio className="size-3.5 shrink-0 animate-pulse" aria-hidden />
      )}
      <span className="font-medium">
        {stopped ? "Stopped — disabled." : "Running."}
      </span>
      {!stopped && runtime.cron ? (
        <span className="text-emerald-700/80 dark:text-emerald-300/80">
          schedule <code className="font-mono">{runtime.cron}</code>
        </span>
      ) : null}
      {!stopped && runtime.nextFireTs ? (
        <span className="flex items-center gap-1" aria-label="next fire">
          <Clock className="size-3" aria-hidden />
          next fire {relativeFuture(runtime.nextFireTs, nowSecs)}
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
      </span>
    </div>
  );
}
