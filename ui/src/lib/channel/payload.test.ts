// Pure unit tests for the kind-tagged payload parser (channels-query-charts scope) — mirrors the
// host's `parse_payload` round-trip (rust/crates/host/src/channel/payload.rs). Asserts that untagged
// bodies stay chat (null) and that each kind round-trips, including the omitted-when-false
// `truncated` field the host drops from the wire.

import { describe, expect, it } from "vitest";

import { parsePayload, encodeQuery } from "./payload.types";

describe("parsePayload", () => {
  it("parses a query body into the tagged union", () => {
    const p = parsePayload(`{"kind":"query","source":"warehouse","sql":"SELECT 1"}`);
    expect(p).toEqual({ kind: "query", source: "warehouse", sql: "SELECT 1" });
  });

  it("plain text is chat (null)", () => {
    expect(parsePayload("hello world")).toBeNull();
  });

  it("json without a kind is chat (null)", () => {
    expect(parsePayload(`{"foo":1}`)).toBeNull();
  });

  it("an unknown kind is chat (null)", () => {
    expect(parsePayload(`{"kind":"chat","text":"hi"}`)).toBeNull();
  });

  it("a query_result round-trips, tolerating an absent truncated (host drops it when false)", () => {
    const body = `{"kind":"query_result","source":"s","sql":"SELECT 1","columns":["v"],"rows":[{"v":1}]}`;
    const p = parsePayload(body);
    expect(p?.kind).toBe("query_result");
    if (p?.kind === "query_result") {
      expect(p.columns).toEqual(["v"]);
      expect(p.truncated).toBeUndefined();
      expect(p.chart).toBeUndefined();
    }
  });

  it("a query_result with a chart spec parses the chart verbatim", () => {
    const body = `{"kind":"query_result","source":"s","sql":"x","columns":["day","n"],"rows":[],"chart":{"type":"line","x":"day","series":[{"field":"n"}]}}`;
    const p = parsePayload(body);
    if (p?.kind === "query_result") {
      expect(p.chart).toEqual({ type: "line", x: "day", series: [{ field: "n" }] });
    } else {
      throw new Error("expected query_result");
    }
  });

  it("a query_error parses its message", () => {
    const p = parsePayload(`{"kind":"query_error","source":"s","sql":"x","error":"not permitted"}`);
    expect(p).toEqual({ kind: "query_error", source: "s", sql: "x", error: "not permitted" });
  });
});

describe("encodeQuery", () => {
  it("builds a kind:query body that round-trips", () => {
    const body = encodeQuery("warehouse", "SELECT 1");
    expect(parsePayload(body)).toEqual({ kind: "query", source: "warehouse", sql: "SELECT 1" });
  });
});
