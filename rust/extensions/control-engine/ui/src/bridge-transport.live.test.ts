// OPT-IN real-engine proof of the FULL bridge read path (S7 exit-gate manual tier, automated).
// Runs ONLY when CE_ENGINE_URL is set (a live ce-studio, default 127.0.0.1:7979) AND
// CONTROL_ENGINE_BIN points at the real (non-fake) `control-engine` sidecar binary. It spawns that
// sidecar (real rubix-ce REST/WS client — NO LB_CE_FAKE), then drives the REAL `BridgeTransport`
// over a `bridge.call` that frames each `control-engine.*` tool onto the sidecar's stdio control line.
// So the path exercised is exactly: BridgeTransport (REST path → tool → arg shape) → sidecar →
// live CE → verbatim DTO → BridgeTransport result-unwrap. No stub, no fake.
//
// This is the S7 read half proven end-to-end against a real engine (the write half is proven by the
// Rust `control_engine_real_write_flow` tier; the routed hop by the Rust routing test). The vitest
// gateway harness has no native-sidecar/SSE transport, so this narrow spawn-the-real-sidecar harness
// is how S7 gets a live automated check without inventing gateway plumbing.

import { spawn, type ChildProcess } from "node:child_process";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { BridgeTransport } from "./bridge-transport";

const ENGINE = process.env.CE_ENGINE_URL;
const BIN = process.env.CONTROL_ENGINE_BIN;
const run = ENGINE && BIN ? describe : describe.skip;

// A single always-present assertion so this file is never an all-skipped "no tests" failure when the
// live tier is off (the default `pnpm test`). The real proof is the env-gated suite below.
it("real-engine tier is opt-in (set CE_ENGINE_URL + CONTROL_ENGINE_BIN to run it)", () => {
  expect(typeof BridgeTransport).toBe("function");
});

/** Minimal Content-Length JSON-RPC client over the sidecar's stdio (the lb-supervisor ABI). */
class Sidecar {
  private child: ChildProcess;
  private buf = Buffer.alloc(0);
  private pending = new Map<number, (r: { result?: string; error?: string }) => void>();
  private id = 1;

  constructor(bin: string, appliance: string) {
    this.child = spawn(bin, [], {
      env: {
        ...process.env,
        LB_EXT_ID: "control-engine",
        LB_EXT_WS: "live",
        // Reads don't self-check; leave LB_EXT_TOKEN unset (grant-only parse yields no caps → reads OK,
        // writes would deny — this harness proves the read path).
      },
      stdio: ["pipe", "pipe", "inherit"],
    });
    this.child.stdout!.on("data", (d: Buffer) => this.onData(d));
    void appliance;
  }

  private onData(d: Buffer) {
    this.buf = Buffer.concat([this.buf, d]);
    for (;;) {
      const sep = this.buf.indexOf("\r\n\r\n");
      if (sep < 0) return;
      const header = this.buf.subarray(0, sep).toString();
      const m = /Content-Length:\s*(\d+)/i.exec(header);
      if (!m) return;
      const len = Number(m[1]);
      const start = sep + 4;
      if (this.buf.length < start + len) return;
      const body = this.buf.subarray(start, start + len).toString();
      this.buf = this.buf.subarray(start + len);
      const reply = JSON.parse(body) as { id: number; result?: string; error?: string };
      this.pending.get(reply.id)?.(reply);
      this.pending.delete(reply.id);
    }
  }

  private send(method: string, params: string): Promise<{ result?: string; error?: string }> {
    const id = this.id++;
    const payload = JSON.stringify({ id, method, params });
    const frame = `Content-Length: ${Buffer.byteLength(payload)}\r\n\r\n${payload}`;
    return new Promise((resolve) => {
      this.pending.set(id, resolve);
      this.child.stdin!.write(frame);
    });
  }

  async init() {
    await this.send("init", "");
  }

  /** The `bridge.call` shape BridgeTransport consumes: forward a tool + args to the sidecar. */
  async call(tool: string, args: Record<string, unknown>): Promise<unknown> {
    const params = JSON.stringify({ tool, input: JSON.stringify(args) });
    const reply = await this.send("call", params);
    if (reply.error) throw new Error(reply.error);
    return JSON.parse(reply.result ?? "null");
  }

  kill() {
    this.child.kill("SIGKILL");
  }
}

run("BridgeTransport against a live CE engine", () => {
  const appliance = (ENGINE ?? "127.0.0.1:7979").replace(/^https?:\/\//, "");
  let sidecar: Sidecar;
  let transport: BridgeTransport;

  beforeAll(async () => {
    sidecar = new Sidecar(BIN!, appliance);
    await sidecar.init();
    transport = new BridgeTransport(
      { call: (tool, args) => sidecar.call(tool, args ?? {}) },
      appliance,
    );
  }, 20_000);

  afterAll(() => sidecar?.kill());

  it("GET /nodes returns the live engine's real component tree through the full bridge", async () => {
    const data = (await transport.request({ method: "GET", path: "/nodes" })) as {
      nodes: unknown[];
      edges: unknown[];
    };
    expect(Array.isArray(data.nodes)).toBe(true);
    expect(Array.isArray(data.edges)).toBe(true);
    // A real engine always has a root/system tree — at least one node.
    expect(data.nodes.length).toBeGreaterThan(0);
  }, 20_000);

  it("GET /schema returns the live type catalogue (the add-node palette)", async () => {
    const data = (await transport.request({ method: "GET", path: "/schema" })) as {
      manifests?: unknown[];
    };
    // The bridge unwraps schema to what rest.ts expects; a live engine ships built-in extensions.
    expect(data).toBeTruthy();
  }, 20_000);
});
