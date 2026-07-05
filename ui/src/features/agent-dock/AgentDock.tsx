// The AGENT DOCK (agent-dock scope) — the persistent, resizable, NON-MODAL right panel, shell-mounted
// beside <Outlet/> so it survives navigation. Composition + wiring only (FILE-LAYOUT): the data lives
// in the session/run hooks; each piece (picker, caption, message list, run status, composer) is its own
// file. Feature seam per the scope's resolved decisions.
//
// Why the `@nube/panel` PRIMITIVES (`useResizable` + `ResizeHandle`) and NOT its `Panel` component: the
// scope requires a non-modal panel that REFLOWS the page (the page shrinks, the user keeps working),
// but `@nube/panel`'s `Panel` wraps a modal `Sheet` (Radix Dialog — overlay + focus trap), which is the
// "Sheet overlay" the scope explicitly rejected. So we build the frame from its non-modal primitives
// (their whole reason to exist) in a shell flex slot. Recorded in the scope doc (open-questions close).

import { useCallback, useEffect, useRef, useState } from "react";
import { Bot, X } from "lucide-react";
import { ResizeHandle, useResizable } from "@nube/panel";

import { pauseRun, resumeRun, stopRun } from "@/lib/channel/run.control";

import { MessageList } from "@/features/channel/MessageList";
import { Button } from "@/components/ui/button";
import { usePageContext } from "./PageContextProvider";
import { useDockSessions } from "./useDockSessions";
import { useDockSession } from "./useDockSession";
import { useDockRun } from "./useDockRun";
import { usePersonaFocus } from "./usePersonaFocus";
import { latestPendingRun } from "./pendingRun";
import { DockSessionPicker } from "./DockSessionPicker";
import { DockPersonaChip } from "./DockPersonaChip";
import { DockContextCaption } from "./DockContextCaption";
import { DockRunStatus } from "./DockRunStatus";
import { DockComposer } from "./DockComposer";
import { DOCK_MAX_WIDTH, DOCK_MIN_WIDTH } from "./useDockChrome";

interface Props {
  ws: string;
  principal: string;
  width: number;
  /** Persist a new width as the user drags. */
  onWidth: (w: number) => void;
  /** Close the dock (the X, or Escape when focused) and return focus to the launcher. */
  onClose: () => void;
  /** Report whether a run is currently in flight — drives the StatusBar launcher's run-state pip. */
  onRunningChange?: (running: boolean) => void;
  /** Injected clock — deterministic in tests. */
  now?: () => number;
}

export function AgentDock({ ws, principal, width, onWidth, onClose, onRunningChange, now }: Props) {
  const sessions = useDockSessions(ws, principal);
  const session = useDockSession(ws, sessions.current, principal, now);
  const page = usePageContext();

  // Resolve the persona focus the chip displays AND the dock sends as the per-invoke `persona` arg
  // (persona-session #5). The live surface (router-derived) feeds the context match; the pin rides
  // in this tab's sessionStorage. The chip and the run must never disagree: `focus.current?.id` is
  // exactly what `ask` will pass as `persona` (undefined when null ⇒ server folds prefs).
  const surface = page.capture().surface;
  const personaFocus = usePersonaFocus(ws, surface);

  const pending = latestPendingRun(session.items);
  // Watch the newest run while it has no durable result/error yet (active). The run stream degrades
  // honestly when `mcp:agent.watch:call` is absent; the durable answer still lands via the channel.
  const active = pending.job != null && !pending.hasResult && !pending.hasError;
  const run = useDockRun(pending.job ?? "", active, pending.hasResult, pending.hasError, now);

  // Run controls (agent-dock run controls): pause/stop/resume the live run over `mcp:agent.control`.
  // `paused` is optimistic UI state keyed to the current run job — set on Pause, cleared on Resume, and
  // reset whenever the run job changes or a terminal result/error lands (so a new run starts clean).
  const [pausedJob, setPausedJob] = useState<string | null>(null);
  const paused = pausedJob !== null && pausedJob === pending.job && active;
  useEffect(() => {
    if (!active) setPausedJob(null); // a durable result/error settled → clear the paused flag
  }, [active]);
  const [controlError, setControlError] = useState<string | null>(null);
  const control = useCallback(
    async (op: "pause" | "stop" | "resume") => {
      if (!pending.job) return;
      try {
        setControlError(null);
        if (op === "pause") {
          await pauseRun(pending.job);
          setPausedJob(pending.job);
        } else if (op === "resume") {
          await resumeRun(pending.job);
          setPausedJob(null);
        } else {
          await stopRun(pending.job);
        }
      } catch (e) {
        setControlError(e instanceof Error ? e.message : String(e));
      }
    },
    [pending.job],
  );

  // Resize: the `@nube/panel` non-modal primitive. Seed with the persisted width; report every change
  // up so it persists across reloads (the chrome hook owns storage).
  const resizable = useResizable({ initial: width, min: DOCK_MIN_WIDTH, max: DOCK_MAX_WIDTH });
  useEffect(() => {
    onWidth(resizable.width);
  }, [resizable.width, onWidth]);

  // Escape closes when focus is inside the dock, returning focus to the launcher (scope: decision 1).
  const rootRef = useRef<HTMLElement>(null);
  useEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    el.addEventListener("keydown", onKey);
    return () => el.removeEventListener("keydown", onKey);
  }, [onClose]);

  const busy = active && run.phase !== "error";

  // Report run-in-flight up to the StatusBar launcher pip (honest: a real pending run, not "open").
  useEffect(() => {
    onRunningChange?.(busy);
    return () => onRunningChange?.(false);
  }, [busy, onRunningChange]);

  return (
    <aside
      ref={rootRef}
      aria-label="agent dock"
      className="lb-panel relative flex h-full shrink-0 flex-col border-l border-border bg-panel"
      style={{ width: resizable.width }}
    >
      <ResizeHandle resizable={resizable} aria-label="resize agent dock" />

      <header className="flex items-center gap-2 border-b border-border bg-panel-2/60 px-3 py-2">
        <Bot size={15} className="shrink-0 text-accent" />
        <span className="text-sm font-semibold text-fg">Agent</span>
        <div className="ml-auto flex min-w-0 items-center gap-2">
          <div className="w-44 min-w-0">
            <DockSessionPicker
              sessions={sessions.sessions}
              current={sessions.current}
              onSelect={sessions.select}
              onNew={sessions.newSession}
            />
          </div>
          <DockPersonaChip focus={personaFocus} />
          <Button
            type="button"
            variant="ghost"
            size="sm"
            aria-label="close agent dock"
            onClick={onClose}
            className="h-8 w-8 shrink-0 p-0"
          >
            <X size={15} />
          </Button>
        </div>
      </header>

      <DockContextCaption context={page.capture()} />

      <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
        {session.loading ? (
          <div className="flex-1 p-4" aria-label="loading dock history">
            <div className="h-10 w-2/3 animate-pulse rounded-md border border-border bg-panel-2" />
          </div>
        ) : (
          <MessageList
            items={session.items}
            author={principal}
            ws={ws}
            onEdit={() => {}}
            onDelete={() => {}}
          />
        )}
      </div>

      {(active || run.phase === "error" || run.degraded) && (
        <div className="border-t border-border bg-panel-2/40 px-3 py-2">
          <DockRunStatus
            phase={run.phase}
            feed={run.feed}
            elapsedSec={run.elapsedSec}
            degraded={run.degraded}
            errorText={pending.errorText}
            paused={paused}
            onRetry={() => pending.goal && void session.ask(pending.goal)}
            onPause={active && !paused ? () => void control("pause") : undefined}
            onStop={active && !paused ? () => void control("stop") : undefined}
            onResume={paused ? () => void control("resume") : undefined}
          />
          {controlError && (
            <p role="alert" className="mt-1 text-xs text-destructive">
              {controlError}
            </p>
          )}
        </div>
      )}

      {(session.error || sessions.error) && (
        <p role="alert" className="border-t border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {session.error ?? sessions.error}
        </p>
      )}

      <DockComposer
        onAsk={(goal) => void session.ask(goal, personaFocus.current?.id)}
        busy={busy}
      />
    </aside>
  );
}
