/**
 * The ingest surface — the durable write path + the read-back. Mirrors
 * `rust/role/gateway/src/routes/ingest.rs` 1:1. The `producer` field of a
 * `Sample` is host-forced to the authenticated principal (un-spoofable), so
 * callers may leave it unset here.
 */

import type { Client } from "./client.js";

/** The canonical `Sample` envelope (see `rust/crates/ingest/src/sample.rs`).
 * `producer` is host-forced; `labels`, `qos` optional. */
export interface Sample {
  series: string;
  /** Host-overridden with the authenticated principal. Omit on the wire. */
  producer?: string;
  ts: number;
  /** Monotonic per (series, producer) — the ordering + dedup key. */
  seq: number;
  payload: unknown;
  labels?: Record<string, unknown>;
  qos?: "best-effort" | "must-deliver";
}

/** `POST /ingest` reply. */
export interface WriteSamplesReply {
  accepted: number;
  committed: number;
}

/** `GET /series/{s}/latest` reply — `sample` is the raw committed envelope, or
 * `null` when the series is empty. */
export interface LatestSampleReply {
  sample: Sample | null;
}

/** Push `samples` to the durable ingest buffer. Returns `{accepted, committed}`
 * — the staged count and the count drained to the committed `series` table on
 * the same call (the gateway node carries the ingest path, so the write is
 * visible to the next read). */
export async function writeSamples(
  client: Client,
  samples: Sample[],
): Promise<WriteSamplesReply> {
  return client.requestJson<WriteSamplesReply>("POST", "/ingest", { samples });
}

/** `GET /series/{series}/latest` — the newest committed sample, or `null` if
 * the series has no samples yet. The simplest read-back proving the round-trip. */
export async function latestSample(
  client: Client,
  series: string,
): Promise<LatestSampleReply> {
  const path = `/series/${encodeURIComponent(series)}/latest`;
  return client.requestJson<LatestSampleReply>("GET", path);
}
