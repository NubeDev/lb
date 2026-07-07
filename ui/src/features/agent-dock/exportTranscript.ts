// Serialize a dock session to markdown for pasting to an external AI (e.g. to debug the agent/backend).
// Pure + one responsibility (FILE-LAYOUT): (context + items) in, a markdown string out — no clipboard,
// no React. The button component owns the copy; this owns the SHAPE, so it is unit-testable.
//
// The payload is the transcript the user sees plus the run context that shapes it: the workspace, the
// active persona focus, and the page surface the ask was made from. Each item's `body` is a kind-tagged
// JSON envelope (`agent`/`agent_result`/…) — we PARSE it into a readable turn (the goal a member asked,
// the answer/error the worker returned, the runtime that served it) rather than dumping raw JSON. That
// is what someone helping improve the agent needs — who acted, in what focus, on what page, what was
// asked, and what came back.
//
// NOTE: the per-turn TOOL CALLS + their results live in the durable run-job record, not in the channel
// item, so they are not available here. A richer export that reads the run record is a follow-up; this
// captures the visible conversation + context, which already surfaces the common failure ("the agent
// says it doesn't have tool X").

import type { Item } from "@/lib/channel/channel.types";
import type { RunToolCall } from "@/features/channel/useRunFeed";
import { parsePayload } from "@/lib/channel/payload.types";

export interface TranscriptContext {
  ws: string;
  /** The signed-in principal (the human author). */
  principal: string;
  /** The active persona id the dock sent as the per-invoke focus, if any. */
  personaId?: string | null;
  /** The page surface the dock captured (router-derived), if any. */
  surface?: string | null;
  /** The LATEST run's tool calls, live-captured from its event stream (older runs' calls live only
   *  in the durable run-job record and are not available here). */
  latestRunTools?: RunToolCall[];
}

/** One rendered turn: a heading and a markdown body. */
interface Turn {
  heading: string;
  body: string;
}

/** Fence multi-line/JSON-ish bodies so they stay readable when pasted; plain prose is left as-is. */
function block(body: string): string {
  const trimmed = body.trim();
  if (!trimmed) return "_(empty)_";
  const looksStructured = trimmed.startsWith("{") || trimmed.startsWith("[") || trimmed.includes("\n");
  return looksStructured ? "```\n" + trimmed + "\n```" : trimmed;
}

/** Turn one channel item into a readable turn, or `null` to skip (empty/unrenderable). */
function itemToTurn(it: Item, principal: string): Turn | null {
  const payload = parsePayload(it.body);

  if (payload?.kind === "agent") {
    return { heading: `🧑 user (${principal}) asked`, body: block(payload.goal) };
  }
  if (payload?.kind === "agent_result") {
    const rt = payload.runtime ? ` · runtime: ${payload.runtime}` : "";
    return { heading: `🤖 agent answered${rt}`, body: block(payload.answer) };
  }
  if (payload?.kind === "agent_error") {
    return { heading: "⚠️ agent error", body: block(payload.error) };
  }

  // An ordinary (untagged) chat message, or a non-agent payload — render the raw body if any.
  const raw = it.body.trim();
  if (!raw) return null;
  const who = it.author === principal ? `🧑 ${principal}` : `💬 ${it.author}`;
  return { heading: who, body: block(raw) };
}

/**
 * Render `items` (oldest→newest, as held) plus `ctx` to a markdown document ready to paste to an AI.
 */
export function exportTranscript(ctx: TranscriptContext, items: Item[]): string {
  const lines: string[] = [];
  lines.push("# Agent dock transcript");
  lines.push("");
  lines.push(`- **workspace:** ${ctx.ws}`);
  lines.push(`- **user:** ${ctx.principal}`);
  lines.push(`- **persona focus:** ${ctx.personaId || "(none — workspace default)"}`);
  lines.push(`- **page surface:** ${ctx.surface || "(none)"}`);
  lines.push("");
  lines.push(
    "_Note: per-run tool calls live in the run-job record, not the channel transcript; the latest " +
      "run's calls (live-captured) are appended below when available._",
  );
  lines.push("");
  lines.push("---");
  lines.push("");

  const turns = items.map((it) => itemToTurn(it, ctx.principal)).filter((t): t is Turn => t !== null);
  if (turns.length === 0) {
    lines.push("_(no messages yet)_");
    return lines.join("\n");
  }

  for (const t of turns) {
    lines.push(`### ${t.heading}`);
    lines.push("");
    lines.push(t.body);
    lines.push("");
  }

  if (ctx.latestRunTools && ctx.latestRunTools.length > 0) {
    lines.push("### 🔧 tool calls (latest run, live-captured)");
    lines.push("");
    for (const t of ctx.latestRunTools) {
      const status = t.err != null ? `✗ ${t.err}` : t.ok !== undefined ? "✓" : "… (still running)";
      lines.push(`- \`${t.name}\` — ${status}`);
    }
    lines.push("");
  }

  return lines.join("\n").trimEnd() + "\n";
}
