// The agent kind-tagged item renderer (channels-agent scope) — turns an `agent` / `agent_result` /
// `agent_error` payload into a CARD, never raw JSON. Sibling of QueryCard:
//   - `agent`        — the user's ask. While the run is live this is the full RunningCard (goal +
//                      spinner + tool calls + streamed text); once the durable `agent_result` /
//                      `agent_error` lands, the live-run chrome is superseded but the GOAL stays on
//                      this card as the user's chat turn (this was the "my message vanished" bug —
//                      returning null hid the only trace of what the user asked).
//   - `agent_result` — the durable answer, attributed to the runtime that served it.
//   - `agent_error`  — an opaque/honest failure (e.g. "agent not permitted").
// RENDER only (FILE-LAYOUT: no data/effects here).

import { AlertTriangle, Bot, Check, Loader2, Pause, Wrench, X } from "lucide-react";

import type {
  AgentErrorPayload,
  AgentPayload,
  AgentResultPayload,
  AgentStalledPayload,
} from "@/lib/channel/payload.types";
import { useRunFeed, type RunToolCall } from "./useRunFeed";
import { MarkdownView } from "./MarkdownView";
import { AnswerCopyButton } from "./AnswerCopyButton";

type AgentKindPayload =
  | AgentPayload
  | AgentResultPayload
  | AgentErrorPayload
  | AgentStalledPayload;

interface Props {
  payload: AgentKindPayload;
  /** True when a durable `agent_result`/`agent_error` for this run already exists in the channel —
   *  then the pending `agent` request's live-run chrome (spinner/tools/streamed text) is superseded.
   *  The user's GOAL still renders as their chat turn (we do NOT return null — that wiped the user's
   *  own message once the answer arrived). */
  settled?: boolean;
}

/** The pending-run card — subscribes to the run's live `RunEvent` feed and shows the agent working
 *  (reasoning, tool calls, streamed text). Falls back to a plain "running…" chip when there is no
 *  gateway feed (Tauri shell / tests) — the durable answer still arrives via the channel. */
function RunningCard({ payload }: { payload: AgentPayload }) {
  const feed = useRunFeed(payload.job, true);
  return (
    <div aria-label="agent request" className="flex flex-col gap-1.5">
      <div className="flex items-center gap-2">
        <RuntimeChip runtime={payload.runtime ?? "default"} />
        <span className="flex items-center gap-1 text-xs text-muted">
          <Loader2 size={12} className="animate-spin" /> {feed.finished ? "finishing…" : "running…"}
        </span>
      </div>
      <p className="text-sm text-fg">{payload.goal}</p>

      {feed.tools.length > 0 && (
        <ul className="flex flex-col gap-0.5" aria-label="agent tool calls">
          {feed.tools.map((t) => (
            <ToolRow key={t.id} tool={t} />
          ))}
        </ul>
      )}
      {feed.reasoning && !feed.text && (
        <p className="line-clamp-2 text-xs italic text-muted">{feed.reasoning}</p>
      )}
      {feed.text && (
        <MarkdownView>{feed.text}</MarkdownView>
      )}
    </div>
  );
}

/** One live tool call: spinner while running, ✓ on success, ✗ on error. */
function ToolRow({ tool }: { tool: RunToolCall }) {
  const done = tool.ok !== undefined || tool.err !== undefined;
  const failed = tool.err != null;
  return (
    <li className="flex items-center gap-1.5 text-xs text-muted">
      <Wrench size={11} className="shrink-0" />
      <code className="truncate font-mono">{tool.name}</code>
      {!done ? (
        <Loader2 size={11} className="shrink-0 animate-spin" />
      ) : failed ? (
        <X size={11} className="shrink-0 text-destructive" />
      ) : (
        <Check size={11} className="shrink-0 text-accent" />
      )}
    </li>
  );
}

/** The user's ask, shown as a chat turn once the durable answer/error has landed. The live-run chrome
 *  (spinner, tool calls, streamed text) belongs to RunningCard and is dropped here — the durable
 *  `agent_result`/`agent_error` card carries the answer/error. The goal stays so the user's message
 *  doesn't vanish when the agent replies. */
function UserGoalCard({ goal }: { goal: string }) {
  return (
    <p aria-label="agent request" className="break-words text-sm leading-6 text-fg">
      {goal}
    </p>
  );
}

/** A small runtime chip — which agent answered (the in-house `default` or an external profile id). */
function RuntimeChip({ runtime }: { runtime: string }) {
  const label = runtime === "default" ? "in-house agent" : runtime;
  return (
    <span className="flex shrink-0 items-center gap-1 rounded-md bg-accent/15 px-2 py-0.5 text-xs font-medium text-accent">
      <Bot size={12} /> {label}
    </span>
  );
}

export function AgentCard({ payload, settled }: Props) {
  if (payload.kind === "agent") {
    // Settled → the live-run chrome is obsolete, but the GOAL is the user's chat turn and must stay.
    // (Returning null here was the "my message vanished after the agent replied" bug.)
    if (settled) return <UserGoalCard goal={payload.goal} />;
    return <RunningCard payload={payload} />;
  }

  if (payload.kind === "agent_stalled") {
    // PAUSE-AND-ASK on the channel surface: the run stalled and was suspended. This card is render-only
    // (FILE-LAYOUT — no control wiring here), so it shows the honest paused state and points to the dock
    // for the Keep going / Stop controls (which own the `agent.control` calls). The goal stays on the
    // `agent` card above as the user's turn.
    return (
      <div
        role="alert"
        className="flex items-start gap-2 rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm"
        aria-label="agent run paused — stalled"
      >
        <Pause size={14} className="mt-0.5 shrink-0 text-amber-500" />
        <p className="min-w-0 break-words text-fg">
          {payload.message} <span className="text-muted">Open the agent dock to keep going or stop.</span>
        </p>
      </div>
    );
  }

  if (payload.kind === "agent_error") {
    // Goal intentionally NOT echoed here: the `agent` card above still shows it as the user's turn,
    // so repeating it would double up. Only the failure surfaces here.
    return (
      <div role="alert" className="flex items-start gap-2 text-sm text-destructive">
        <AlertTriangle size={14} className="mt-0.5 shrink-0" />
        <p className="min-w-0 break-words">{payload.error}</p>
      </div>
    );
  }

  // agent_result — the durable answer. Goal intentionally NOT echoed: the `agent` card above still
  // shows it as the user's chat turn.
  return (
    <div aria-label="agent result" className="flex flex-col gap-1.5">
      <div className="flex items-center gap-2">
        <RuntimeChip runtime={payload.runtime} />
        <AnswerCopyButton text={payload.answer} className="ml-auto" />
      </div>
      <MarkdownView>{payload.answer}</MarkdownView>
      {payload.truncated && (
        <p className="text-xs text-muted">(answer truncated — see the full run)</p>
      )}
    </div>
  );
}
