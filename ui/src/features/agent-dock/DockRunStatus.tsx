// The dock RUN-STATUS strip (agent-dock scope, the feedback contract) — renders ONE of the six run
// states as an honest, labelled line: never a bare spinner. Presentation only (FILE-LAYOUT: no data /
// effects); the phase + fields come from `useDockRun`, retry is a callback.
//
//   Sent → "Sent" (connecting)   Working → live activity + elapsed   Answering → streamed text
//   Stalled → "still working" hint (not an error)   Done → nothing (the durable answer is the record)
//   Error → the message + a Retry affordance.

import { useState } from "react";
import { AlertTriangle, Check, Loader2, Pause, Play, RotateCcw, Square, WifiOff, Wrench, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { RunFeed, RunToolCall } from "@/features/channel/useRunFeed";
import type { DockRunPhase } from "./dockRunState";

/** How many tool rows to show before FIFO-collapsing the rest. Oldest calls are hidden first so the
 *  newest (incl. the in-flight one) always stay visible during a long run. Mirrors the same shape as
 *  `SURFACE_PREVIEW_COUNT` in ExtensionsView — keep the dock compact without losing the audit (the
 *  full list is one "Show all" click away). */
const MAX_VISIBLE_TOOLS = 5;

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
  /** True when the run STALLED (no progress) and was suspended awaiting a keep-going/stop decision —
   *  a durable, server-authoritative pause (distinct from the optimistic user `paused`). */
  stalled?: boolean;
  /** The honest prompt text from the `agent_stalled` item (why it may be stuck). */
  stalledText?: string | null;
  onRetry: () => void;
  /** Pause the live run (suspend). Absent → the controls are hidden (no `agent.control` wiring). */
  onPause?: () => void;
  /** Stop (cancel) the live run — terminal. */
  onStop?: () => void;
  /** Resume a paused run. */
  onResume?: () => void;
  /** Keep going after a stall — resume the suspended run from its cursor. */
  onKeepGoing?: () => void;
  /** Stop after a stall — cancel the suspended run (terminal). */
  onStopStalled?: () => void;
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
  stalled,
  stalledText,
  onRetry,
  onPause,
  onStop,
  onResume,
  onKeepGoing,
  onStopStalled,
}: Props) {
  // STALLED — the run made no progress and was suspended (server-authoritative pause-and-ask). Show an
  // honest explanation + an explicit choice: Keep going (resume from the cursor) or Stop (cancel). Takes
  // precedence over the live phase (the stream ended when the run suspended). Distinct from `paused`
  // (the user's own optimistic pause): a stall is the system asking the user to decide.
  if (stalled && phase !== "done" && phase !== "error") {
    return (
      <div
        role="alert"
        className="flex items-start gap-2 rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm"
        aria-label="run stalled — awaiting your decision"
      >
        <Pause size={14} className="mt-0.5 shrink-0 text-amber-500" />
        <div className="min-w-0 flex-1">
          <p className="break-words text-fg">
            {stalledText || "The agent hasn't made progress for a while — it may be stuck."}
          </p>
          <div className="mt-2 flex items-center gap-2">
            {onKeepGoing && (
              <Button
                type="button"
                size="sm"
                onClick={onKeepGoing}
                aria-label="keep going"
                className="h-7 gap-1 px-2.5 text-xs"
              >
                <Play size={12} /> Keep going
              </Button>
            )}
            {onStopStalled && (
              <Button
                type="button"
                size="sm"
                variant="ghost"
                onClick={onStopStalled}
                aria-label="stop run"
                className="h-7 gap-1 px-2.5 text-xs text-destructive hover:bg-destructive/10 hover:text-destructive"
              >
                <Square size={11} /> Stop
              </Button>
            )}
          </div>
        </div>
      </div>
    );
  }
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
    // The durable agent_result is the message of record (rendered by the message list). What it does
    // NOT carry is the run's tool calls — keep the live-captured list visible so the user can see
    // WHAT the agent actually did (the #1 "did it really do it?" question), plus the degrade note.
    if (!degraded && feed.tools.length === 0) return null;
    return (
      <div className="flex flex-col gap-1">
        <ToolList tools={feed.tools} />
        {degraded && <DegradeNote />}
      </div>
    );
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
      <ToolList tools={feed.tools} />
      {degraded && <DegradeNote />}
    </div>
  );
}

/** The run's tool calls so far, one honest row each: done (✓), failed (✗), or still running. The
 *  durable channel item never carries these, so this live-captured list is the only place the user
 *  sees what the agent actually did. Renders nothing before the first call.
 *
 *  When the list outgrows {@link MAX_VISIBLE_TOOLS}, the OLDEST calls are hidden first (FIFO) and a
 *  "Show all" toggle reveals them — keeps the dock height stable during a long run without losing the
 *  audit. The cap is presentation-only: `useDockRun`'s `feed.tools` keeps every call. */
function ToolList({ tools }: { tools: RunToolCall[] }) {
  const [expanded, setExpanded] = useState(false);
  if (tools.length === 0) return null;
  const overflow = tools.length > MAX_VISIBLE_TOOLS;
  const hiddenCount = overflow ? tools.length - MAX_VISIBLE_TOOLS : 0;
  const visible = overflow && !expanded ? tools.slice(hiddenCount) : tools;
  return (
    <div className="flex flex-col gap-0.5">
      {overflow && (
        <div className="flex items-center gap-1.5 text-xs text-muted">
          {!expanded ? (
            <>
              <span className="min-w-0 truncate">{hiddenCount} earlier calls hidden</span>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                aria-expanded={false}
                aria-label="show all tool calls"
                onClick={() => setExpanded(true)}
                className="ml-auto h-6 px-2 text-xs text-muted hover:text-fg"
              >
                Show all
              </Button>
            </>
          ) : (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              aria-expanded={true}
              aria-label="show fewer tool calls"
              onClick={() => setExpanded(false)}
              className="ml-auto h-6 px-2 text-xs text-muted hover:text-fg"
            >
              Show fewer
            </Button>
          )}
        </div>
      )}
      <ul className="flex flex-col gap-0.5" aria-label="tool calls">
        {visible.map((t) => (
          <li key={t.id} className="flex items-center gap-1.5 text-xs text-muted">
            {t.err != null ? (
              <X size={11} className="shrink-0 text-destructive" />
            ) : t.ok !== undefined ? (
              <Check size={11} className="shrink-0 text-emerald-500" />
            ) : (
              <Loader2 size={11} className="shrink-0 animate-spin" />
            )}
            <code className="truncate font-mono text-[11px]">{t.name}</code>
            {t.err != null && <span className="min-w-0 truncate text-destructive">{t.err}</span>}
          </li>
        ))}
      </ul>
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
