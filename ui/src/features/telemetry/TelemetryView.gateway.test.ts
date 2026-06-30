// The telemetry console over a REAL gateway (telemetry-console scope; CLAUDE §9 — no fake backend).
// Seeds REAL telemetry rows through the real write path (`/_seed/telemetry` → `lb_host::telemetry_seed`
// → `capped_insert` + tail publish, the same two ops the `SurrealCappedLayer` performs), then drives
// the SHIPPED console data layer (`telemetry.query`/`trace` over `mcp_call`, and the SSE tail over
// `/telemetry/stream`). Covers the scope's mandatory cases:
//   - snapshot rows read back + filters NARROW (source / level / text / trace pivot),
//   - capability-deny: a session WITHOUT `telemetry:read` is refused (opaque), no rows,
//   - workspace-isolation: ws-B reads ONLY ws-B rows (the read-surface wall),
//   - a LIVE row arrives over the SSE tail (a real frame, read via fetch+reader — jsdom has no
//     EventSource, so the live feed is asserted at the transport, exactly like the Studio build-log test).

import { describe, expect, it, beforeAll } from "vitest";
import { inject } from "vitest";

import { queryTelemetry, traceTelemetry } from "@/lib/telemetry";
import { sessionToken } from "@/lib/session/session.store";
import {
  useRealGateway,
  signInReal,
  signInWithCaps,
  seedTelemetry,
} from "@/test/gateway-session";

let n = 0;
const nextWs = () => `tel-console-${n++}`;

beforeAll(() => useRealGateway());

describe("telemetry console (real gateway)", () => {
  it("reads seeded rows back and NARROWS by source / level / text", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedTelemetry({ source: "host", level: "info", tool: "doc.read", msg: "all good here", traceId: "tr1" });
    await seedTelemetry({ source: "host", level: "warn", outcome: "deny", tool: "doc.delete", msg: "denied access", traceId: "tr1" });
    await seedTelemetry({ source: "mqtt", level: "error", outcome: "error", tool: "mqtt.publish", msg: "broker down", traceId: "tr2" });

    // Unfiltered: all three.
    const all = await queryTelemetry({}, 50);
    expect(all.rows.length).toBe(3);

    // source = mqtt → only the mqtt row.
    const bySource = await queryTelemetry({ source: "mqtt" }, 50);
    expect(bySource.rows.map((r) => r.source)).toEqual(["mqtt"]);

    // level ≥ error → only the error row.
    const byLevel = await queryTelemetry({ level: "error" }, 50);
    expect(byLevel.rows.map((r) => r.level)).toEqual(["error"]);

    // free-text "denied" → only the deny row.
    const byText = await queryTelemetry({ text: "denied" }, 50);
    expect(byText.rows.length).toBe(1);
    expect(byText.rows[0].outcome).toBe("deny");

    // trace pivot: tr1 correlates its two rows; tr2 has one.
    const tr1 = await traceTelemetry("tr1");
    expect(tr1.length).toBe(2);
    const tr2 = await traceTelemetry("tr2");
    expect(tr2.length).toBe(1);
  });

  it("capability-deny: a session WITHOUT telemetry:read is refused (opaque, no rows)", async () => {
    const ws = nextWs();
    // A real signed session that holds an unrelated grant but NOT `mcp:telemetry.read:call`.
    await signInWithCaps("user:dave", ws, ["mcp:inbox.list:call"]);
    await expect(queryTelemetry({}, 50)).rejects.toThrow();
    await expect(traceTelemetry("whatever")).rejects.toThrow();
  });

  it("workspace-isolation: ws-B reads ONLY ws-B rows (the read-surface wall)", async () => {
    const wsA = nextWs();
    const wsB = `other-${wsA}`;
    await signInReal("user:ada", wsA);
    await seedTelemetry({ source: "host", msg: "secret A activity", tool: "a.tool" });
    await seedTelemetry({ source: "host", msg: "more A activity", tool: "a.tool" });

    await signInReal("user:bob", wsB);
    await seedTelemetry({ source: "host", msg: "B activity", tool: "b.tool" });

    // ws-B session reads — sees ONLY its own row, never ws-A's (the ws is the token, server-side).
    const page = await queryTelemetry({}, 50);
    expect(page.rows.length).toBe(1);
    expect(page.rows[0].tool).toBe("b.tool");
    expect(page.rows.some((r) => r.msg.includes("A activity"))).toBe(false);
  });

  it("a LIVE row arrives over the SSE tail", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    // Open the tail, THEN seed a row — it must arrive as a live `telemetry` frame (the snapshot is
    // empty at attach, so a row we see is genuinely live motion the Layer/seed published).
    const rows = readTailUntil(ws, 1, async () => {
      // Give the subscription a moment to attach before publishing.
      await new Promise((r) => setTimeout(r, 250));
      await seedTelemetry({ source: "live", tool: "live.tool", msg: "a live row", traceId: "live-1" });
    });
    const got = await rows;
    expect(got.some((r) => (r as { source?: string }).source === "live")).toBe(true);
  });
});

/** Read the telemetry SSE tail (snapshot + live frames) until `n` frames arrive (or timeout), running
 *  `after` once the stream is open so the test can publish a live row. Uses fetch+reader because jsdom
 *  has no EventSource — the same transport-level pattern the Studio build-log test uses. */
function readTailUntil(
  _ws: string,
  n: number,
  after: () => Promise<void>,
): Promise<unknown[]> {
  const base = inject("gatewayUrl");
  const url = `${base}/telemetry/stream?token=${encodeURIComponent(sessionToken())}`;
  return (async () => {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 20_000);
    const frames: unknown[] = [];
    let fired = false;
    try {
      const res = await fetch(url, { signal: controller.signal });
      if (!res.ok || !res.body) throw new Error(`stream failed: ${res.status}`);
      // Fire the publisher once the stream is established.
      if (!fired) {
        fired = true;
        void after();
      }
      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let buffer = "";
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });
        for (;;) {
          const split = buffer.indexOf("\n\n");
          if (split < 0) break;
          const frame = buffer.slice(0, split);
          buffer = buffer.slice(split + 2);
          const line = dataLine(frame);
          if (!line) continue;
          try {
            frames.push(JSON.parse(line));
          } catch {
            /* a malformed frame never breaks the read */
          }
          if (frames.length >= n) return frames;
        }
      }
      return frames;
    } finally {
      clearTimeout(timer);
      controller.abort();
    }
  })();
}

/** Extract the `data:` payload from an SSE frame (ignoring `event:`/comment lines). */
function dataLine(frame: string): string | null {
  for (const raw of frame.split("\n")) {
    if (raw.startsWith("data:")) return raw.slice(5).trim();
  }
  return null;
}
