// Vitest **globalSetup** for the real-gateway tests (data-console scope; the start of retiring the
// `*.fake.ts` backend — CLAUDE §9, testing §0). It spawns the REAL gateway-role node (the
// `test_gateway` bin in `role/gateway`) on an OS-assigned port, waits until it is listening, and
// hands the base URL to the test process via Vitest's `provide`/`inject` channel. Teardown kills it.
//
// This is the smallest real-node harness the scope requires: UI behaviour is proven against a real
// backend over its real HTTP transport, seeded with real rows through the real write path — never a
// hand-written fake. Tests that opt into it import `gatewayUrl()` (below) and drive the real `invoke`
// HTTP path by pointing the session + `VITE_GATEWAY_URL` at the spawned server.

import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { GlobalSetupContext } from "vitest/node";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// The workspace root is three levels up from ui/src/test.
const REPO = path.resolve(__dirname, "../../..");
const RUST = path.join(REPO, "rust");
const BIN = path.join(RUST, "target/debug/test_gateway");

let child: ChildProcess | null = null;
let weatherStub: http.Server | null = null;

/** A canned Open-Meteo `current=` body every `weather.gateway.test.tsx` case reads (weather scope).
 *  A real local HTTP server, not a mocked client (CLAUDE §9) — the ONE sanctioned external fake-
 *  boundary, shared across the whole gateway suite since the spawned node is a single long-lived
 *  process (its env is fixed at spawn time, so this can't vary per test). */
// `time` is a UTC epoch (SECONDS) — the node requests `timeformat=unixtime`. 1783598400 =
// 2026-07-09T12:00:00Z (the UI renders it in the viewer's browser timezone).
const WEATHER_STUB_BODY = JSON.stringify({
  current: { time: 1783598400, temperature_2m: 21.4, wind_speed_10m: 11.2, weather_code: 3 },
});

/** Serve the canned weather body on an ephemeral loopback port; returns its base URL. */
function startWeatherStub(): Promise<string> {
  return new Promise((resolve) => {
    weatherStub = http.createServer((_req, res) => {
      res.writeHead(200, { "content-type": "application/json" });
      res.end(WEATHER_STUB_BODY);
    });
    weatherStub.listen(0, "127.0.0.1", () => {
      const addr = weatherStub!.address();
      const port = typeof addr === "object" && addr ? addr.port : 0;
      resolve(`http://127.0.0.1:${port}`);
    });
  });
}

export default async function setup({ provide }: GlobalSetupContext) {
  // Build the test-only harness binary first (gated behind the `test-harness` feature so its seed
  // deps don't touch the production crate). Cheap when already built.
  const build = spawnSync(
    "cargo",
    ["build", "-p", "lb-role-gateway", "--features", "test-harness", "--bin", "test_gateway"],
    { cwd: RUST, stdio: "inherit" },
  );
  if (build.status !== 0) throw new Error("failed to build test_gateway harness binary");

  const weatherBase = await startWeatherStub();

  const url = await new Promise<string>((resolve, reject) => {
    child = spawn(BIN, [], {
      env: {
        ...process.env,
        PORT: "0",
        // Login-hardening: `Gateway::boot()` selects its credential check from the env —
        // unset `LB_DEV_LOGIN` → `PasswordHash` (a real argon2 secret required), which would
        // `401` the password-less `signInReal` login. The harness is dev/CI, so opt into the
        // `DevTrustAny` (password-less) check the same way `make dev` does.
        LB_DEV_LOGIN: "1",
        LB_DEVKIT_ROOT: path.join(RUST, "extensions"),
        LB_DIR: path.join(RUST, "target", "devkit-gateway-lb"),
        // weather scope: point `weather.current` at the local stub above instead of the real
        // Open-Meteo — every gateway test gets the SAME canned reading (the node is one shared
        // long-lived process, so this can't be varied per test).
        LB_WEATHER_OPEN_METEO_BASE: weatherBase,
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

  // Hand the URL to every test file via Vitest's typed inject channel.
  provide("gatewayUrl", url);

  return () => {
    child?.kill("SIGKILL");
    weatherStub?.close();
  };
}

// Augment Vitest's ProvidedContext so `inject("gatewayUrl")` is typed in the tests.
declare module "vitest" {
  export interface ProvidedContext {
    gatewayUrl: string;
  }
}
