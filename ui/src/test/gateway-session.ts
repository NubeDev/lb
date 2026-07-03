// Shared helper for the real-gateway tests: point `invoke` at the spawned server and log in for a
// real signed session. Importing this from a `*.gateway.test.tsx` gives a `signInReal(workspace)`
// that mints a real token (carrying the dev claim set — which includes the data-console caps) and
// stores it, so every subsequent `invoke` call hits the real backend with a real bearer token.

import { inject, vi } from "vitest";

import { login } from "@/lib/session/session.api";
import { setSession, sessionToken } from "@/lib/session/session.store";
import type { Session } from "@/lib/session/session.types";

/** Make `invoke` take the real HTTP path to the spawned gateway (stub `VITE_GATEWAY_URL` to its URL).
 *  Call once per test file (idempotent). */
export function useRealGateway(): void {
  vi.stubEnv("VITE_GATEWAY_URL", inject("gatewayUrl"));
}

/** Log in `user` into `workspace` against the real gateway and store the session. Returns the session
 *  (token + principal + caps). A unique workspace per test keeps the shared real backend isolated. */
export async function signInReal(user: string, workspace: string) {
  const session = await login(user, workspace);
  setSession(session);
  return session;
}

/** Mint and store a real signed session from the test gateway with an explicit cap set. This is used
 *  for deny tests where dev-login's broad cap set would be too privileged. */
export async function signInWithCaps(user: string, workspace: string, caps: string[]): Promise<Session> {
  const url = inject("gatewayUrl");
  const res = await fetch(`${url}/_seed/session`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ user, workspace, caps }),
  });
  if (!res.ok) throw new Error(`seed session failed: ${res.status} ${await res.text()}`);
  const session = (await res.json()) as Session;
  setSession(session);
  return session;
}

/** POST to a test-only `/_seed/*` route on the spawned gateway, authenticated by the current session
 *  token (so the seed lands in the session's workspace — the real write path, behind the workspace
 *  wall). For surfaces with no public create route (inbox item / outbox effect / extension install). */
async function seed(
  kind:
    | "inbox"
    | "outbox"
    | "extension"
    | "iot_demo"
    | "series"
    | "proof_panel"
    | "flow_node"
    | "telemetry"
    | "agent_drain",
  body: unknown,
): Promise<void> {
  const url = inject("gatewayUrl");
  const token = sessionToken();
  const res = await fetch(`${url}/_seed/${kind}`, {
    method: "POST",
    headers: { "content-type": "application/json", authorization: `Bearer ${token}` },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`seed ${kind} failed: ${res.status} ${await res.text()}`);
}

/** Seed ONE real telemetry row into the session workspace through the REAL write path
 *  (`lb_host::telemetry_seed` → `capped_insert` + tail publish — the same two ops the
 *  `SurrealCappedLayer` performs). The console reads it back over `telemetry.query`/`tail`. All fields
 *  default sensibly; pass what the assertion needs (source/level/outcome/traceId/msg). Never a fake row. */
export function seedTelemetry(row: {
  level?: string;
  actor?: string;
  tool?: string;
  source?: string;
  traceId?: string;
  outcome?: string;
  ts?: number;
  msg?: string;
}): Promise<void> {
  return seed("telemetry", {
    level: row.level,
    actor: row.actor,
    tool: row.tool,
    source: row.source,
    trace_id: row.traceId,
    outcome: row.outcome,
    ts: row.ts,
    msg: row.msg,
  });
}

/** Seed the dashboard demo series (`cooler.temp`/`fryer.state`) + tags into the session workspace,
 *  through the real ingest path (dashboard scope). Lets a dashboard test bind widgets to real series. */
export function seedIotDemo(): Promise<void> {
  return seed("iot_demo", {});
}

/** Drive the session workspace's channel-agent run queue to completion (the real
 *  `drain_channel_agent_runs` the production reactor calls — the test gateway doesn't spawn the timer,
 *  so a UI test flushes on demand). After this resolves, a posted `kind:"agent"` request's durable
 *  `agent_result`/`agent_error` is in the channel history. Driving through the real path, not faking. */
export function drainAgentRuns(): Promise<void> {
  return seed("agent_drain", {});
}

/** Seed a real durable inbox item into the session workspace. */
export function seedInbox(item: {
  id: string;
  channel: string;
  author: string;
  body: string;
  ts: number;
  meta?: Record<string, unknown>;
}): Promise<void> {
  return seed("inbox", item);
}

/** Seed a real outbox effect into the session workspace. */
export function seedOutbox(effect: {
  id: string;
  target: string;
  action: string;
  payload?: string;
  idempotency_key?: string;
  status?: string;
  attempts?: number;
  max_attempts?: number;
  next_attempt_ts?: number;
  ts: number;
}): Promise<void> {
  return seed("outbox", {
    effect: {
      payload: "",
      idempotency_key: effect.id,
      status: "pending",
      attempts: 0,
      max_attempts: 5,
      next_attempt_ts: 0,
      ...effect,
    },
  });
}

/** Seed ONE discoverable series into the session workspace through the REAL write path: a committed
 *  sample value (so `series.latest` reads it) + a `key:value` tag edge on the `series:<name>` entity
 *  (so `series.find` discovers it). Used by the proof-panel real-gateway test — never a fake row. */
export function seedSeries(s: {
  series: string;
  seq: number;
  payload: unknown;
  key: string;
  value: unknown;
}): Promise<void> {
  return seed("series", s);
}

/** Install AND LOAD the REAL proof-panel wasm component into the session workspace, so its
 *  `proof-panel.proof.derive` tool is callable over the live bridge (host-callback scope). Unlike
 *  `seedExtension` (which only writes an Install record), this loads the component into the runtime. */
export function seedProofPanel(): Promise<void> {
  return seed("proof_panel", {});
}

/** Seed a real extension install into the session workspace. */
export function seedExtension(ext: {
  ext: string;
  version: string;
  tier?: "wasm" | "native";
  enabled?: boolean;
  ui?: { entry: string; label: string; icon?: string; scope?: string[] };
  widgets?: { entry: string; label: string; icon?: string; scope?: string[]; data?: boolean }[];
  /** Native only: seed a durable native_status with this restart count (surfaces the count + the
   *  Reset affordance in the console without a live child). */
  restart_count?: number;
}): Promise<void> {
  return seed("extension", ext);
}

/** Install a real extension that contributes ONE `[[node]]` to the session workspace, so `flows.nodes`
 *  returns it (the palette + ext-node tests). Writes a real Install record carrying the node block +
 *  the granted tool cap (exactly the path a real install persists) — seeding, not faking. The body is
 *  remapped to the host's raw snake_case `SeedFlowNode` shape. */
export function seedFlowNode(node: {
  ext: string;
  /** The `mcp:<ext>.<tool>:call` cap to grant (the node's bound tool). */
  toolCap: string;
  /** The `[[node]]` block fields (type/kind/tool + optional title/category/config_version/config). */
  block: Record<string, unknown>;
}): Promise<void> {
  return seed("flow_node", {
    ext: node.ext,
    tool_cap: node.toolCap,
    node: node.block,
  });
}
