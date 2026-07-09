// The live **agent-run** feed over SSE (agent-run scope Part 3) — mirrors the gateway's
// `GET /runs/{job}/stream?token=`. The channel AgentCard opens this once per pending run and receives
// the agent's `RunEvent`s (reasoning, tool calls, text) as it works, instead of only a final answer.
//
// One verb: `openRunStream`. Uses the native `EventSource`, so it only runs in a real browser against a
// real gateway — in the Tauri shell / tests there is no gateway URL and the caller skips it (there the
// answer still arrives via the channel post→refresh / SSE round trip). Sibling of `channel.stream.ts`.

import { eventHub, liveStreamAvailable } from "@/lib/events/hub";

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
 *  walls the run subject by workspace, so a ws-B session can never observe a ws-A run.
 *
 *  `onError` (agent-dock scope) fires on an `EventSource` error — a 403 (no `mcp:agent.watch:call` /
 *  cross-workspace), a 401, or a dropped/killed stream. The dock uses it to degrade honestly (no live
 *  deltas, the durable answer still renders) and to surface a killed-stream error with retry. The
 *  callback is additive: existing callers that pass only `onEvent` are unaffected. */
export function openRunStream(
  job: string,
  onEvent: (event: RunEvent) => void,
  onError?: () => void,
): RunStream | null {
  if (!liveStreamAvailable()) return null;
  // Delegates to the shared event hub (unified-event-stream scope): the `run:{job}` subject rides the
  // one multiplexed connection. The frame shape is unchanged — the gateway wraps the SAME `event: run`
  // payload in the mux envelope, and the hub hands it back verbatim. The gate (`mcp:agent.watch:call`)
  // still runs on subscribe; a deny arrives as an opaque `error` frame, which we surface via `onError`.
  const unsubscribe = eventHub.subscribeSubject(`run:${job}`, (frame) => {
    if (frame.event === "run") {
      try {
        onEvent(JSON.parse(frame.data) as RunEvent);
      } catch {
        // a malformed frame never breaks the stream
      }
    } else if (frame.event === "error") {
      onError?.();
    }
  });
  return { close: unsubscribe };
}
