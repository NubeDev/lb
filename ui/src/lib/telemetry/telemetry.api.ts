// The telemetry console API client — one call per export, over the host-mediated MCP bridge
// (`mcp_call` → `POST /mcp/call`), mirroring the host's `telemetry.*` read verbs. The UI never calls
// `invoke` raw; it goes through these named verbs (FILE-LAYOUT frontend rules). Every call is
// capability-gated server-side (`mcp:telemetry.read:call`) and HARD-filtered to the caller's
// workspace — the `ws` is never an argument (the wall, §7).
//
// `telemetry.tail` is NOT here — the live feed rides the SSE route (`telemetry.stream.ts`), not an
// in-band tool result. There is no `telemetry.write` (rows come from the Layer only).

import { invoke } from "@/lib/ipc/invoke";
import type {
  TelemetryFilter,
  TelemetryPage,
  TelemetryRow,
} from "./telemetry.types";

/** Map the host's snake_case stored row to the camelCase view type. The stored body uses
 *  `trace_id`/`params_digest`; everything else is already flat. */
export function normalizeRow(raw: Record<string, unknown>): TelemetryRow {
  return {
    seq: String(raw.seq ?? ""),
    level: String(raw.level ?? ""),
    ws: String(raw.ws ?? ""),
    actor: String(raw.actor ?? ""),
    tool: String(raw.tool ?? ""),
    source: String(raw.source ?? ""),
    traceId: String(raw.trace_id ?? raw.traceId ?? ""),
    outcome: String(raw.outcome ?? ""),
    ts: Number(raw.ts ?? 0),
    msg: String(raw.msg ?? ""),
    paramsDigest:
      raw.params_digest != null ? String(raw.params_digest) : undefined,
    fields: (raw.fields as Record<string, unknown> | undefined) ?? undefined,
  };
}

/** Encode the camelCase filter to the host's snake_case args. Empty fields are omitted so the host
 *  applies only the active clauses. */
function encodeFilter(filter: TelemetryFilter): Record<string, unknown> {
  const args: Record<string, unknown> = {};
  if (filter.source) args.source = filter.source;
  if (filter.actor) args.actor = filter.actor;
  if (filter.level) args.level = filter.level;
  if (filter.outcome) args.outcome = filter.outcome;
  if (filter.traceId) args.trace_id = filter.traceId;
  if (filter.text) args.text = filter.text;
  if (filter.since != null) args.since = filter.since;
  if (filter.until != null) args.until = filter.until;
  return args;
}

/** A filtered, paged snapshot of the workspace's telemetry ring, newest-first. Mirrors
 *  `telemetry.query`. `cursor` is the `seq` of the previous page's oldest row (null for the first). */
export async function queryTelemetry(
  filter: TelemetryFilter,
  limit = 100,
  cursor?: string | null,
): Promise<TelemetryPage> {
  const args = encodeFilter(filter);
  args.limit = limit;
  if (cursor) args.cursor = cursor;
  const res = await invoke<{ rows: Record<string, unknown>[]; next: string | null }>(
    "mcp_call",
    { tool: "telemetry.query", args },
  );
  return { rows: (res.rows ?? []).map(normalizeRow), next: res.next ?? null };
}

/** Every row sharing a `traceId` in this workspace — the timeline pivot. Mirrors `telemetry.trace`. */
export async function traceTelemetry(traceId: string): Promise<TelemetryRow[]> {
  const res = await invoke<{ rows: Record<string, unknown>[] }>("mcp_call", {
    tool: "telemetry.trace",
    args: { trace_id: traceId },
  });
  return (res.rows ?? []).map(normalizeRow);
}

/** Clear the workspace's ring (node-admin). Mirrors `telemetry.purge`; returns the count removed. */
export async function purgeTelemetry(): Promise<number> {
  const res = await invoke<{ removed: number }>("mcp_call", {
    tool: "telemetry.purge",
    args: {},
  });
  return res.removed ?? 0;
}
