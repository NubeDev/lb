// The dock RUN-STATUS strip (agent-dock scope, the feedback contract) — renders ONE of the six run
// states as an honest, labelled line: never a bare spinner. Presentation only (FILE-LAYOUT: no data /
// effects); the phase + fields come from `useDockRun`, retry is a callback.
//
//   Sent → "Sent" (connecting)   Working → live activity + elapsed   Answering → streamed text
//   Stalled → "still working" hint (not an error)   Done → nothing (the durable answer is the record)
//   Error → the message + a Retry affordance.

import { AlertTriangle, Loader2, Pause, Play, RotateCcw, Square, WifiOff, Wrench } from "lucide-react";

import type { RunFeed } from "@/features/channel/useRunFeed";
import type { DockRunPhase } from "./dockRunState";

interface Props {
  phase: DockRunPhase;
  feed: RunFeed;
  elapsedSec: number;
  /** True when the live progress stream was denied/dropped — show the honest "no live progress" note. */
  degraded: boolean;
  /** The error message to show in the Error state (a durable agent_error, or a transport failure). */
  errorText?: string | null;
  /** True when the user has paused the run (optimistic) — show Resume instead of Pause/Stop. */
  paused?: boolean;
  onRetry: () => void;
  /** Pause the live run (suspend). Absent → the controls are hidden (no `agent.control` wiring). */
  onPause?: () => void;
  /** Stop (cancel) the live run — terminal. */
  onStop?: () => void;
  /** Resume a paused run. */
  onResume?: () => void;
}

/** The live activity line: the current tool call, else the reasoning line, else a generic "thinking". */
function activityLabel(feed: RunFeed): string {
  const running = feed.tools.find((t) => t.ok === undefined && t.err === undefined);
  if (running) return `calling ${running.name}…`;
  if (feed.reasoning) return "thinking…";
  return "thinking…";
}

export function DockRunStatus({
  phase,
  feed,
  elapsedSec,
  degraded,
  errorText,
  paused,
  onRetry,
  onPause,
  onStop,
  onResume,
}: Props) {
  // PAUSED — the user suspended the run. A distinct, honest state (not a spinner, not an error): show
  // it's paused + a Resume button. Takes precedence over the live phase (the stream may have ended).
  if (paused && phase !== "done" && phase !== "error") {
    return (
      <div className="flex items-center gap-2 text-xs text-muted" aria-label="run paused">
        <Pause size={12} className="shrink-0 text-amber-500" />
        <span className="min-w-0 flex-1 truncate">Paused</span>
        {onResume && (
          <button
            type="button"
            onClick={onResume}
            aria-label="resume run"
            className="inline-flex items-center gap-1 rounded-sm px-1.5 py-0.5 text-accent hover:bg-accent/10"
          >
            <Play size={12} /> Resume
          </button>
        )}
      </div>
    );
  }

  if (phase === "done") {
    // The durable agent_result is the message of record (rendered by the message list); nothing to add
    // beyond an optional degrade note.
    return degraded ? <DegradeNote /> : null;
  }

  if (phase === "error") {
    return (
      <div
        role="alert"
        className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive"
      >
        <AlertTriangle size={14} className="mt-0.5 shrink-0" />
        <div className="min-w-0 flex-1">
          <p className="break-words">{errorText || "The agent run failed."}</p>
          <button
            type="button"
            onClick={onRetry}
            className="mt-1.5 inline-flex items-center gap-1 rounded-sm text-xs font-medium text-destructive underline-offset-2 hover:underline"
          >
            <RotateCcw size={12} /> Retry
          </button>
        </div>
      </div>
    );
  }

  // Live states: Sent / Working / Answering / Stalled — one line, with elapsed once the run is going.
  const line =
    phase === "sent"
      ? "Sent — connecting…"
      : phase === "answering"
        ? "Answering…"
        : phase === "stalled"
          ? "Still working — this is taking a while…"
          : activityLabel(feed);

  return (
    <div className="flex flex-col gap-1" aria-label={`run ${phase}`} aria-live="polite">
      <div className="flex items-center gap-2 text-xs text-muted">
        {phase === "stalled" ? (
          <Loader2 size={12} className="shrink-0 animate-spin text-amber-500" />
        ) : phase === "working" && feed.tools.some((t) => t.ok === undefined && t.err === undefined) ? (
          <Wrench size={12} className="shrink-0" />
        ) : (
          <Loader2 size={12} className="shrink-0 animate-spin" />
        )}
        <span className="min-w-0 flex-1 truncate">{line}</span>
        {phase !== "sent" && <span className="shrink-0 tabular-nums">{elapsedSec}s</span>}
        {/* Live controls — pause (suspend, resumable) + stop (cancel, terminal). Shown for ANY live
            phase (incl. the pre-delta "sent" — the run job may already be driving server-side). Hidden
            only when no control handlers are wired (no `agent.control` grant / not active). */}
        {(onPause || onStop) && (
          <span className="flex shrink-0 items-center gap-0.5">
            {onPause && (
              <button
                type="button"
                onClick={onPause}
                aria-label="pause run"
                title="Pause"
                className="inline-flex h-5 w-5 items-center justify-center rounded-sm hover:bg-panel-2 hover:text-fg"
              >
                <Pause size={12} />
              </button>
            )}
            {onStop && (
              <button
                type="button"
                onClick={onStop}
                aria-label="stop run"
                title="Stop"
                className="inline-flex h-5 w-5 items-center justify-center rounded-sm hover:bg-destructive/10 hover:text-destructive"
              >
                <Square size={11} />
              </button>
            )}
          </span>
        )}
      </div>
      {degraded && <DegradeNote />}
    </div>
  );
}

/** The honest "no live progress" note shown when the caller lacks `mcp:agent.watch:call` (or the
 *  stream dropped) — the answer still arrives durably; only the live deltas are gone. */
function DegradeNote() {
  return (
    <p className="flex items-center gap-1.5 text-xs text-muted">
      <WifiOff size={11} className="shrink-0" />
      No live progress (missing agent.watch) — the answer will still appear.
    </p>
  );
}
