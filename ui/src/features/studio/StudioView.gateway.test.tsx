// Extension Studio over a REAL spawned gateway. This drives the SDK chain through the public bridge:
// templates -> scaffold -> build with SSE logs -> server-side publish -> call generated tool.

import { rm } from "node:fs/promises";

import { beforeAll, describe, expect, inject, it } from "vitest";

import {
  buildDevkitExtension,
  inspectDevkitExtension,
  listDevkitTemplates,
  publishDevkitExtension,
  scaffoldDevkitExtension,
} from "@/lib/devkit/devkit.api";
import { invoke } from "@/lib/ipc/invoke";
import { sessionToken } from "@/lib/session/session.store";
import { signInWithCaps, useRealGateway } from "@/test/gateway-session";

let n = 0;
const nextId = () => `studio-gateway-${Date.now()}-${n++}`;

beforeAll(() => useRealGateway());

describe("Extension Studio (real gateway)", () => {
  it("scaffolds, builds, streams logs, publishes, and calls the generated wasm tool", async () => {
    const id = nextId();
    await signInWithCaps("user:studio", `studio-${n}`, [
      "mcp:devkit.templates:call",
      "mcp:devkit.scaffold:call",
      "mcp:devkit.inspect:call",
      "mcp:devkit.build:call",
      "mcp:ext.publish:call",
      "mcp:ext.list:call",
      "mcp:bus.watch:call",
      `mcp:${id}.ping:call`,
    ]);

    const templates = await listDevkitTemplates();
    expect(templates.map((t) => t.tier)).toContain("wasm");

    const scaffold = await scaffoldDevkitExtension({
      id,
      tier: "wasm",
      features: [],
    });
    try {
      expect(scaffold.path).toContain(id);
      const started = await buildDevkitExtension(scaffold.path);
      const lines = await collectBuildLog(started.log_subject);
      expect(lines.some((line) => line.includes("cargo"))).toBe(true);
      expect(lines).toContain("devkit build: done");

      const inspected = await inspectDevkitExtension(scaffold.path);
      expect(inspected.built).toBe(true);

      await publishDevkitExtension(scaffold.path);
      const out = await invoke<{ ok: boolean; ext: string; tier: string }>(
        "mcp_call",
        {
          tool: `${id}.ping`,
          args: {},
        },
      );
      expect(out).toEqual({ ok: true, ext: id, tier: "wasm" });
    } finally {
      await rm(scaffold.path, { recursive: true, force: true });
    }
  }, 120_000);
});

function collectBuildLog(subject: string): Promise<string[]> {
  const base = inject("gatewayUrl");
  const url = `${base}/bus/stream?subject=${encodeURIComponent(subject)}&token=${encodeURIComponent(
    sessionToken(),
  )}`;
  return readSseLines(url);
}

async function readSseLines(url: string): Promise<string[]> {
  const controller = new AbortController();
  const timer = window.setTimeout(() => controller.abort(), 120_000);
  const lines: string[] = [];
  try {
    const res = await fetch(url, { signal: controller.signal });
    if (!res.ok || !res.body) throw new Error(`stream failed: ${res.status}`);
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
        lines.push(line);
        if (line === "devkit build: done") return lines;
        if (line === "devkit build: failed") throw new Error(lines.join("\n"));
      }
    }
    throw new Error("build stream ended before terminal frame");
  } finally {
    window.clearTimeout(timer);
    controller.abort();
  }
}

function dataLine(frame: string): string | null {
  const data = frame
    .split("\n")
    .filter((line) => line.startsWith("data:"))
    .map((line) => line.slice("data:".length).trim())
    .join("\n");
  if (!data) return null;
  const value = JSON.parse(data) as unknown;
  return typeof value === "string" ? value : null;
}
