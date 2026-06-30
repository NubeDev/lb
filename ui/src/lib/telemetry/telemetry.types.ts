// View/DTO types for the telemetry console (telemetry-console scope). Mirror the Rust stored event
// schema (`lb_telemetry::TelemetryRecord` / `lb_host::telemetry::query::TelemetryRow`): the flat,
// queryable fields the console filters on. There is NO raw-params field here — params reach the row
// only as `paramsDigest` (a SHA-256 + shape summary; the secret never leaves the host, §6.7).

/** The bounded log level the console filters on (no free-form severity). */
export type TelemetryLevel = "error" | "warn" | "info" | "debug" | "trace";

/** The capability-decision outcome the console filters on (the security-relevant dimension). */
export type TelemetryOutcome = "allow" | "deny" | "error";

/** One stored telemetry row, as the console renders it. `seq` is the ULID insert-sequence (FIFO key)
 *  used for stable, newest-first paging. `ws` is always the caller's own — the read surface is
 *  workspace-walled server-side. */
export interface TelemetryRow {
  seq: string;
  level: TelemetryLevel | string;
  ws: string;
  actor: string;
  tool: string;
  source: string;
  traceId: string;
  outcome: TelemetryOutcome | string;
  ts: number;
  msg: string;
  /** The redacted params digest (`<sha256>:<shape>`) — never the raw value. */
  paramsDigest?: string;
  fields?: Record<string, unknown>;
}

/** The composable console filters. All optional; the workspace wall is applied server-side, never
 *  here. `level` is a MINIMUM severity ("level ≥ X"); `text` is a case-insensitive substring on msg. */
export interface TelemetryFilter {
  source?: string;
  actor?: string;
  level?: TelemetryLevel;
  outcome?: TelemetryOutcome;
  traceId?: string;
  text?: string;
  /** Inclusive lower bound on `ts` (the host's logical clock). */
  since?: number;
  /** Exclusive upper bound on `ts`. */
  until?: number;
}

/** A paged snapshot result: rows newest-first + the `seq` cursor for the next (older) page (null at
 *  the end). */
export interface TelemetryPage {
  rows: TelemetryRow[];
  next: string | null;
}

/** Which lane the console is showing. Telemetry is the evictable operational ring; Audit is the
 *  immutable, hash-chained mutation ledger — a SEPARATE store, never merged into the ring. */
export type TelemetryLane = "telemetry" | "audit";
