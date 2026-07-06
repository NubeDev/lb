// Query-draft streaming, driven against a REAL in-process gateway (query-draft-streaming scope;
// CLAUDE §9 / testing §0 — no fake backend). An agent-shaped caller publishes full
// `SqlSourceState` frames through the REAL `bus.publish` verb; the mounted `QueryWorkbench`
// follows over the REAL `GET /bus/stream` SSE (via the fetch-backed EventSource shim — jsdom has
// no EventSource; the shim is a browser-API polyfill against the real backend, not a fake).
//
// Mandatory categories (scope testing-scope §2):
//   - HEADLINE: publish a builder frame → the workbench editor follows (table select updates, the
//     SQL preview shows the frame's query, the "live draft" indicator appears).
//   - CAPABILITY-DENY (§2.1): a session without `mcp:bus.watch:call` gets 403 from `/bus/stream`
//     BEFORE any body — the workbench mounts and works, it just never follows (honest degrade).
//   - WORKSPACE-ISOLATION (§2.2): a publish in ws-B on the SAME subject never reaches ws-A's
//     follower (the host walls subjects to `ws/{id}/ext/…` from the token).
//   - Malformed frame: junk on the subject is dropped — no crash, no state change.

import { describe, expect, it, beforeAll, afterAll, inject } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import { QueryWorkbench } from "@/features/query-workbench";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";
import { installEventSourceShim } from "@/test/eventsource-shim";
import { invoke } from "@/lib/ipc/invoke";
import { setSession, sessionToken } from "@/lib/session/session.store";
import type { SqlSourceState } from "@/lib/panel-kit/sql/query";
import { draftSubject } from "./queryDraft";

let n = 0;
const nextWs = () => `qdraft-${n++}`;

beforeAll(() => useRealGateway());
let uninstall: () => void;
beforeAll(() => {
  uninstall = installEventSourceShim();
});
afterAll(() => uninstall());

/** Seed real samples so `store.schema` reports the `series` table (a `<select>` whose value has no
 *  matching option reports `""` — the schema must actually contain the frame's table). */
async function seedRows(series: string): Promise<void> {
  const samples = Array.from({ length: 3 }, (_, i) => ({
    series,
    producer: "user:ada",
    seq: i + 1,
    payload: (i + 1) * 10,
    ts: i + 1,
  }));
  await invoke("mcp_call", { tool: "ingest.write", args: { samples } });
}

/** Publish one draft frame on `source`'s subject AS the current session (the agent-shaped path —
 *  the same `mcp:bus.publish:call` any MCP caller takes). */
async function publishFrame(source: string, frame: unknown): Promise<void> {
  await invoke("mcp_call", {
    tool: "bus.publish",
    args: { subject: draftSubject(source), payload: frame },
  });
}

const SOURCE = "surreal-local";

const builderFrame = (table: string): SqlSourceState => ({
  mode: "builder",
  rawSql: "",
  builder: { table, columns: [], filters: [], groupBy: [] },
  format: "table",
});

describe("query-draft streaming (real gateway)", () => {
  it("HEADLINE: a published builder frame drives the mounted workbench (editor follows + indicator)", async () => {
    await signInReal("user:ada", nextWs());
    await seedRows("series");
    render(<QueryWorkbench ws="w" sel={null} onSel={() => {}} source={SOURCE} />);
    // The workbench starts empty (no table picked).
    const table = (await screen.findByLabelText("sql table")) as HTMLSelectElement;
    expect(table.value).toBe("");

    // The stream subscription races the publish — publish until a frame lands (fire-and-forget
    // bus: a frame sent before the subscriber attaches is legitimately dropped).
    await waitFor(
      async () => {
        await publishFrame(SOURCE, builderFrame("series"));
        expect(screen.getByLabelText("live draft indicator")).toBeTruthy();
      },
      { timeout: 10_000 },
    );

    // The frame replaced the editor state: builder table + SQL preview follow.
    await waitFor(() => {
      expect((screen.getByLabelText("sql table") as HTMLSelectElement).value).toBe("series");
      expect(screen.getByLabelText("sql preview").textContent).toContain("series");
    });
  });

  it("drops a malformed frame (no crash, no state change) then follows the next valid one", async () => {
    await signInReal("user:ada", nextWs());
    await seedRows("series");
    render(<QueryWorkbench ws="w" sel={null} onSel={() => {}} source={SOURCE} />);
    await screen.findByLabelText("sql table");

    await waitFor(
      async () => {
        // Junk first — must be dropped silently…
        await publishFrame(SOURCE, { mode: "yolo", nonsense: true });
        await publishFrame(SOURCE, "not even an object");
        // …then a valid frame that must still apply.
        await publishFrame(SOURCE, builderFrame("series"));
        expect(screen.getByLabelText("sql preview").textContent).toContain("series");
      },
      { timeout: 10_000 },
    );
  });

  it("CAPABILITY-DENY: without mcp:bus.watch:call the stream 403s pre-body and the workbench degrades silently", async () => {
    const ws = nextWs();
    // A real signed session WITHOUT the watch cap (it can still use the store, so the workbench works).
    await signInWithCaps("user:carol", ws, [
      "mcp:store.query:call",
      "mcp:store.schema:call",
    ]);
    // The stream route denies BEFORE any body (the contract the silent degrade rests on).
    const url = inject("gatewayUrl");
    const res = await fetch(
      `${url}/bus/stream?subject=${encodeURIComponent(draftSubject(SOURCE))}&token=${encodeURIComponent(sessionToken())}`,
    );
    expect(res.status).toBe(403);

    // The workbench still mounts and renders its editor — no crash, merely no following.
    render(<QueryWorkbench ws="w" sel={null} onSel={() => {}} source={SOURCE} />);
    await screen.findByLabelText("sql table");
    expect(screen.queryByLabelText("live draft indicator")).toBeNull();
  });

  it("WORKSPACE-ISOLATION: a publish in ws-B never reaches ws-A's follower on the same subject", async () => {
    const wsA = nextWs();
    const wsB = nextWs();
    const sessionA = await signInReal("user:ada", wsA);
    render(<QueryWorkbench ws="w" sel={null} onSel={() => {}} source={SOURCE} />);
    await screen.findByLabelText("sql table");
    // Give the ws-A stream a moment to attach, then publish from ws-B on the SAME subject.
    await new Promise((r) => setTimeout(r, 500));
    await signInReal("user:bob", wsB);
    await publishFrame(SOURCE, builderFrame("series"));
    await publishFrame(SOURCE, builderFrame("series"));
    // Restore ws-A's session (the mounted stream kept ws-A's token) and assert nothing arrived.
    setSession(sessionA);
    await new Promise((r) => setTimeout(r, 1_000));
    expect(screen.queryByLabelText("live draft indicator")).toBeNull();
    expect((screen.getByLabelText("sql table") as HTMLSelectElement).value).toBe("");
  });
});
