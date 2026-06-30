// Unit tests for the telemetry filter URL codec + the live-row filter match (telemetry-console scope).
// Pure logic — no gateway needed. Asserts the round-trip is stable (shareable links restore the view)
// and that an out-of-band/invalid value never injects a bad clause.

import { describe, expect, it } from "vitest";

import { decodeFilterFromQuery, encodeFilterToQuery } from "./filterUrl";
import { matchesFilter } from "./useTelemetry";
import type { TelemetryFilter, TelemetryRow } from "@/lib/telemetry";

const row = (over: Partial<TelemetryRow> = {}): TelemetryRow => ({
  seq: "01",
  level: "info",
  ws: "w",
  actor: "user:ada",
  tool: "doc.read",
  source: "host",
  traceId: "tr1",
  outcome: "allow",
  ts: 10,
  msg: "all good here",
  ...over,
});

describe("filter URL codec", () => {
  it("round-trips every field", () => {
    const f: TelemetryFilter = {
      source: "mqtt",
      actor: "user:bob",
      level: "warn",
      outcome: "deny",
      traceId: "tr-9",
      text: "boom",
      since: 100,
      until: 200,
    };
    expect(decodeFilterFromQuery(encodeFilterToQuery(f))).toEqual(f);
  });

  it("omits empty fields (short, clean link)", () => {
    expect(encodeFilterToQuery({ source: "mqtt" })).toBe("source=mqtt");
    expect(encodeFilterToQuery({})).toBe("");
  });

  it("drops an invalid level/outcome (bounded set; no bad clause from a hand-edited link)", () => {
    const f = decodeFilterFromQuery("level=nope&outcome=bogus&source=host");
    expect(f.level).toBeUndefined();
    expect(f.outcome).toBeUndefined();
    expect(f.source).toBe("host");
  });
});

describe("matchesFilter (live-row fold)", () => {
  it("source / outcome / text narrow", () => {
    expect(matchesFilter(row(), { source: "host" })).toBe(true);
    expect(matchesFilter(row(), { source: "mqtt" })).toBe(false);
    expect(matchesFilter(row({ outcome: "deny" }), { outcome: "deny" })).toBe(true);
    expect(matchesFilter(row(), { text: "GOOD" })).toBe(true); // case-insensitive
    expect(matchesFilter(row(), { text: "nope" })).toBe(false);
  });

  it("level is a MINIMUM severity (warn matches warn+error, not info)", () => {
    expect(matchesFilter(row({ level: "error" }), { level: "warn" })).toBe(true);
    expect(matchesFilter(row({ level: "warn" }), { level: "warn" })).toBe(true);
    expect(matchesFilter(row({ level: "info" }), { level: "warn" })).toBe(false);
  });

  it("time range bounds (since inclusive, until exclusive)", () => {
    expect(matchesFilter(row({ ts: 10 }), { since: 10 })).toBe(true);
    expect(matchesFilter(row({ ts: 9 }), { since: 10 })).toBe(false);
    expect(matchesFilter(row({ ts: 10 }), { until: 10 })).toBe(false);
  });
});
