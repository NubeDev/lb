// The live **agent-run** feed over SSE (agent-run scope Part 3) — mirrors the gateway's
// `GET /runs/{job}/stream?token=`. The channel AgentCard opens this once per pending run and receives
// the agent's `RunEvent`s (reasoning, tool calls, text) as it works, instead of only a final answer.
//
// One verb: `openRunStream`. Uses the native `EventSource`, so it only runs in a real browser against a
// real gateway — in the Tauri shell / tests there is no gateway URL and the caller skips it (there the
// answer still arrives via the channel post→refresh / SSE round trip). Sibling of `channel.stream.ts`.

import { gatewayUrl } from "@/lib/ipc/http";
import { sessionToken } from "@/lib/session/session.store";

/** One observable thing in a run — mirrors the Rust `RunEvent` (`#[serde(tag="type", kebab-case)]`,
 *  `rust/crates/run-events/src/event.rs`), so the wire `type` values are kebab-case. */
export type RunEvent =
  | { type: "run-start"; goal: string }
  | { type: "step-start"; turn: number }
  | { type: "text-delta"; turn: number; text: string }
  | { type: "reasoning-delta"; turn: number; text: string }
  | { type: "tool-call-start"; id: string; name: string }
  | { type: "tool-call-args-delta"; id: string; delta: string }
  | { type: "tool-call-result"; id: string; ok?: string | null; err?: string | null }
  | { type: "run-finish"; outcome: string; answer: string };

/** A live stream handle — call `close()` to stop (the hook does this on unmount / when settled). */
export interface RunStream {
  close: () => void;
}

/** Open the SSE run feed for `job`. Returns `null` when no gateway is configured (Tauri shell / tests)
 *  — the caller simply has no live feed there, by design (the durable answer still arrives). The token
 *  rides as `?token=` (EventSource can't set headers); the gateway checks `mcp:agent.watch:call` and
 *  walls the run subject by workspace, so a ws-B session can never observe a ws-A run. */
export function openRunStream(job: string, onEvent: (event: RunEvent) => void): RunStream | null {
  const base = gatewayUrl();
  if (base === "" && import.meta.env.VITE_GATEWAY_URL === undefined) return null;
  if (typeof EventSource === "undefined") return null;

  const url = `${base}/runs/${encodeURIComponent(job)}/stream?token=${encodeURIComponent(
    sessionToken(),
  )}`;
  const es = new EventSource(url);

  // The gateway emits `event: run` frames, each carrying one JSON-encoded RunEvent.
  es.addEventListener("run", (e) => {
    try {
      onEvent(JSON.parse((e as MessageEvent).data) as RunEvent);
    } catch {
      // a malformed frame never breaks the stream
    }
  });

  return { close: () => es.close() };
}
