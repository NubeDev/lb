// The dock run-state fold (agent-dock scope, the feedback contract) — collapse the live run signals
// into ONE of the six visible states the dock renders. Never a bare spinner: every state is a distinct,
// honest label so a slow answer never reads as broken.
//
//   Sent      — item posted, run stream connecting (no events yet).
//   Working   — run-start / reasoning / tool-call arriving (live activity + elapsed).
//   Answering — text-delta(s) arriving (the answer is streaming in).
//   Stalled   — stream open but quiet ≥15 s (a hint, NOT an error).
//   Done      — a durable agent_result reconciled (the message of record).
//   Error     — a post reject / 401 / 403 / agent_error item / EventSource error (with retry).
//
// FILE-LAYOUT: a pure reducer, no React — unit-testable. The card component maps a state to markup.

import type { RunFeed } from "@/features/channel/useRunFeed";

export type DockRunPhase =
  | "sent"
  | "working"
  | "answering"
  | "stalled"
  | "done"
  | "error";

/** The inputs that decide the phase — folded run feed + terminal signals + the stall flag. */
export interface DockRunInputs {
  /** The folded live run feed (text/reasoning/tools/finished), or null before a stream opens. */
  feed: RunFeed | null;
  /** True once the durable `agent_result` for this run landed in channel history (the record answer). */
  hasResult: boolean;
  /** True once a durable `agent_error` item landed, OR a transport error occurred (post/stream). */
  hasError: boolean;
  /** True when the stall timer has tripped (live but quiet ≥15 s). */
  stalled: boolean;
}

/** Reduce the inputs to a single phase. Terminal states win (Done/Error are the message of record);
 *  then streaming (text ⇒ Answering); then activity (Working); Stalled overrides Working when quiet;
 *  Sent is the pre-stream default. Pure + total. */
export function dockRunPhase({ feed, hasResult, hasError, stalled }: DockRunInputs): DockRunPhase {
  // TERMINAL first — a durable result/error is the record and supersedes any live state.
  if (hasError) return "error";
  if (hasResult) return "done";

  // Streaming the answer text is the most specific live state.
  if (feed && feed.text.length > 0) return "answering";

  // Live but quiet past the threshold — an honest hint, not an error (server owns the true timeout).
  if (stalled) return "stalled";

  // Any activity short of text: reasoning, a tool call, or a finished feed awaiting the durable item.
  if (feed && (feed.reasoning.length > 0 || feed.tools.length > 0 || feed.finished)) {
    return "working";
  }

  // Posted, stream connecting, nothing observed yet.
  return "sent";
}

/** Whether a phase is terminal (the run is over — stop the stall timer, close the stream). */
export function isTerminalPhase(phase: DockRunPhase): boolean {
  return phase === "done" || phase === "error";
}
