// The flow canvas primary controls (flow-deploy-ux scope) — the Node-RED operator model, extracted
// from FlowCanvas so each file owns one responsibility (FILE-LAYOUT; the canvas was over the 400-line
// limit). Presentational: every action is a prop; all state lives in the canvas/hooks.
//
// The controls, and exactly what each means:
//   • Deploy   — `flows.save`. The ONLY path that pushes canvas edits to the running system. Enabled
//                ONLY when the canvas differs from the deployed flow (`dirty`); disabled when clean, so
//                the operator always knows whether there are undeployed changes.
//   • Run/Stop — `flows.run` / `flows.cancel`. Manual run + mid-run cancel (unchanged).
//   • Suspend/Resume — live-run lifecycle, shown only while a run is in flight.
//   • Enable/Disable — `flows.enable`. "Should this flow ever fire" (durable). Disable = never runs
//                again until re-enabled. Distinct from Deploy (which pushes the GRAPH).
//   • Live values — a toggle gating the observe cost (SSE watch + node_state/runs poll). Off = the
//                canvas paints the last snapshot statically and opens no stream.

import { Pause, Play, Power, Radio, Save, Square } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";

export interface FlowToolbarProps {
  /** The canvas differs from the deployed flow → Deploy is enabled (and highlighted). */
  dirty: boolean;
  /** A run is in flight → show Suspend/Resume/Stop and disable Run. */
  runActive: boolean;
  /** The flow fires on its own (cron/source trigger). When true, Run is a one-off **Test run** — the
   *  real 24/7 firing comes from Enable; when false (manual only), Run is the only way it ever runs. */
  scheduled: boolean;
  /** The durable enabled flag (from node_state; the flow record until it loads). */
  enabled: boolean;
  /** Live-value painting on/off. */
  liveValues: boolean;
  onDeploy: () => void;
  onRun: () => void;
  onLifecycle: (op: "suspend" | "resume" | "cancel") => void;
  onToggleEnabled: () => void;
  onToggleLiveValues: (next: boolean) => void;
}

export function FlowToolbar({
  dirty,
  runActive,
  scheduled,
  enabled,
  liveValues,
  onDeploy,
  onRun,
  onLifecycle,
  onToggleEnabled,
  onToggleLiveValues,
}: FlowToolbarProps) {
  return (
    <>
      <Button
        aria-label="deploy flow"
        onClick={onDeploy}
        disabled={!dirty}
        size="sm"
        className={cn("gap-1.5", dirty && "animate-pulse")}
        title={dirty ? "Deploy your changes to the running flow" : "No changes to deploy"}
      >
        <Save size={13} />
        {dirty ? "Deploy" : "Deployed"}
      </Button>
      <Button
        aria-label={scheduled ? "test run flow" : "run flow"}
        onClick={onRun}
        disabled={runActive}
        variant="outline"
        size="sm"
        className="gap-1.5"
        title={
          scheduled
            ? "Fire this flow once now (a test). Its real 24/7 firing comes from Enable + its trigger."
            : "Run this flow once now"
        }
      >
        <Play size={13} />
        {scheduled ? "Test run" : "Run"}
      </Button>
      {runActive ? (
        <>
          <Button aria-label="suspend run" onClick={() => onLifecycle("suspend")} variant="outline" size="sm" className="gap-1.5">
            <Pause size={13} />
            Suspend
          </Button>
          <Button aria-label="resume run" onClick={() => onLifecycle("resume")} variant="outline" size="sm" className="gap-1.5">
            <Play size={13} />
            Resume
          </Button>
          <Button aria-label="stop run" onClick={() => onLifecycle("cancel")} variant="destructive" size="sm" className="gap-1.5">
            <Square size={13} />
            Stop
          </Button>
        </>
      ) : null}
      <div className="mx-1 h-5 w-px bg-border" />
      <Button
        aria-label={enabled ? "disable flow" : "enable flow"}
        onClick={onToggleEnabled}
        variant={enabled ? "outline" : "default"}
        size="sm"
        className="gap-1.5"
        title={
          enabled
            ? "Disable: the flow stops firing (durable — survives restart)"
            : "Enable: the flow fires on its triggers again"
        }
      >
        <Power size={13} />
        {enabled ? "Disable" : "Enable"}
      </Button>
      <label className="ml-1 flex items-center gap-1.5 text-xs text-muted-foreground" title="Paint each wire's current value (SSE + poll)">
        <Radio size={13} className={cn(liveValues && "text-emerald-500")} aria-hidden />
        <span>Live values</span>
        <Switch
          aria-label="toggle live values"
          checked={liveValues}
          onCheckedChange={onToggleLiveValues}
        />
      </label>
    </>
  );
}
