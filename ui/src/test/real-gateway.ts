// Vitest **globalSetup** for the real-gateway tests (data-console scope; the start of retiring the
// `*.fake.ts` backend — CLAUDE §9, testing §0). It spawns the REAL gateway-role node (the
// `test_gateway` bin in `role/gateway`) on an OS-assigned port, waits until it is listening, and
// hands the base URL to the test process via Vitest's `provide`/`inject` channel. Teardown kills it.
//
// This is the smallest real-node harness the scope requires: UI behaviour is proven against a real
// backend over its real HTTP transport, seeded with real rows through the real write path — never a
// hand-written fake. Tests that opt into it import `gatewayUrl()` (below) and drive the real `invoke`
// HTTP path by pointing the session + `VITE_GATEWAY_URL` at the spawned server.

import { spawn, type ChildProcess } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { GlobalSetupContext } from "vitest/node";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// The workspace root is three levels up from ui/src/test.
const REPO = path.resolve(__dirname, "../../..");
const BIN = path.join(REPO, "rust/target/debug/test_gateway");

let child: ChildProcess | null = null;

export default async function setup({ provide }: GlobalSetupContext) {
  const url = await new Promise<string>((resolve, reject) => {
    child = spawn(BIN, [], { env: { ...process.env, PORT: "0" } });
    const timer = setTimeout(() => reject(new Error("gateway did not start in time")), 20_000);

    child.stdout?.on("data", (buf: Buffer) => {
      const m = /LISTENING (http:\/\/\S+)/.exec(buf.toString());
      if (m) {
        clearTimeout(timer);
        resolve(m[1]);
      }
    });
    child.on("error", (e) => {
      clearTimeout(timer);
      reject(e);
    });
    child.on("exit", (code) => {
      if (code !== 0 && code !== null) reject(new Error(`gateway exited early (${code})`));
    });
  });

  // Hand the URL to every test file via Vitest's typed inject channel.
  provide("gatewayUrl", url);

  return () => {
    child?.kill("SIGKILL");
  };
}

// Augment Vitest's ProvidedContext so `inject("gatewayUrl")` is typed in the tests.
declare module "vitest" {
  export interface ProvidedContext {
    gatewayUrl: string;
  }
}
