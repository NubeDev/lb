// The agent kind-tagged item renderer (channels-agent scope) — turns an `agent` / `agent_result` /
// `agent_error` payload into a CARD, never raw JSON. Sibling of QueryCard:
//   - `agent`        — the request, shown as a "running" chip (the answer streams/arrives as a
//                      separate `agent_result` item; until then this is the live placeholder).
//   - `agent_result` — the durable answer, attributed to the runtime that served it.
//   - `agent_error`  — an opaque/honest failure (e.g. "agent not permitted").
// RENDER only (FILE-LAYOUT: no data/effects here).

import { AlertTriangle, Bot, Check, Loader2, Wrench, X } from "lucide-react";

import type {
  AgentErrorPayload,
  AgentPayload,
  AgentResultPayload,
} from "@/lib/channel/payload.types";
import { useRunFeed, type RunToolCall } from "./useRunFeed";

type AgentKindPayload = AgentPayload | AgentResultPayload | AgentErrorPayload;

interface Props {
  payload: AgentKindPayload;
  /** True when a durable `agent_result`/`agent_error` for this run already exists in the channel —
   *  then the pending `agent` request card is superseded and hidden (no duplicate/orphan spinner). */
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
        <p className="whitespace-pre-wrap break-words text-sm text-fg/90">{feed.text}</p>
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
    // The request. Once its result/error has landed we hide this placeholder (settled).
    if (settled) return null;
    return <RunningCard payload={payload} />;
  }

  if (payload.kind === "agent_error") {
    return (
      <div role="alert" className="flex items-start gap-2 text-sm text-destructive">
        <AlertTriangle size={14} className="mt-0.5 shrink-0" />
        <div className="min-w-0">
          <p className="text-xs text-muted">{payload.goal}</p>
          <p className="mt-1">{payload.error}</p>
        </div>
      </div>
    );
  }

  // agent_result — the durable answer.
  return (
    <div aria-label="agent result" className="flex flex-col gap-1.5">
      <div className="flex items-center gap-2">
        <RuntimeChip runtime={payload.runtime} />
        <span className="truncate text-xs text-muted">{payload.goal}</span>
      </div>
      <p className="whitespace-pre-wrap break-words leading-6 text-fg">{payload.answer}</p>
      {payload.truncated && (
        <p className="text-xs text-muted">(answer truncated — see the full run)</p>
      )}
    </div>
  );
}
