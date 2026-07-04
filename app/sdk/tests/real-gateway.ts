// Vitest globalSetup: build + spawn the REAL gateway-role node (the `test_gateway` bin in
// `role/gateway`) and provide its URL. The app mirror of `ui/src/test/real-gateway.ts` — same bin,
// same seed routes, different consumer (the RN shell's client seam instead of the web `invoke`).

import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { GlobalSetupContext } from "vitest/node";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// The workspace root is two levels up from app/sdk/tests.
const REPO = path.resolve(__dirname, "../../..");
const RUST = path.join(REPO, "rust");
const BIN = path.join(RUST, "target/debug/test_gateway");

let child: ChildProcess | null = null;

export default async function setup({ provide }: GlobalSetupContext) {
  const build = spawnSync(
    "cargo",
    ["build", "-p", "lb-role-gateway", "--features", "test-harness", "--bin", "test_gateway"],
    { cwd: RUST, stdio: "inherit" },
  );
  if (build.status !== 0) throw new Error("failed to build test_gateway harness binary");

  const url = await new Promise<string>((resolve, reject) => {
    child = spawn(BIN, [], {
      env: {
        ...process.env,
        PORT: "0",
        LB_DEVKIT_ROOT: path.join(RUST, "extensions"),
        LB_DIR: path.join(RUST, "target", "app-sdk-gateway-lb"),
      },
    });
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

  provide("gatewayUrl", url);

  return () => {
    child?.kill("SIGKILL");
  };
}

declare module "vitest" {
  export interface ProvidedContext {
    gatewayUrl: string;
  }
}
