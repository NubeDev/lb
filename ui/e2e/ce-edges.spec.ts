// E2E regression: the Control Engine wiresheet renders its EDGE (wire) connections.
//
// Regression for the "wires don't show" bug: `buildRfEdges` produced the edge correctly and
// both endpoints' handles rendered, but the promotion gate (a `useStore` selector racing a
// fixed 1500ms grace timer) never re-fired on a cold federated-bundle load, so React Flow
// was handed ZERO edges and the canvas showed no wires. Fixed by driving edge promotion off
// `useNodesInitialized()` + an rAF poll that reads handle bounds fresh (CeEditor.tsx). See
// docs/debugging/frontend/ce-edges-never-render-readykey-race.md.
//
// Rule 9 (no mocks): this drives the REAL federated CE page in a REAL browser against the
// REAL node + the running ce-studio engine. A jsdom unit test cannot reproduce the race — it
// depends on real handle MEASUREMENT timing, which only a layout engine produces.

import { test, expect } from "@playwright/test";

const SHELL = process.env.LB_SHELL_URL ?? "http://127.0.0.1:4173";
const GATEWAY = process.env.VITE_GATEWAY_URL ?? "http://127.0.0.1:8080";

test.beforeAll(async ({ request }) => {
  const shell = await request.get(SHELL).catch(() => null);
  if (!shell || !shell.ok()) {
    throw new Error(`shell not reachable at ${SHELL} — run 'make ui-preview' (or point LB_SHELL_URL at the dev server)`);
  }
  const remote = await request.get(`${GATEWAY}/extensions/control-engine/ui/remoteEntry.js`).catch(() => null);
  if (!remote || !remote.ok()) {
    throw new Error("control-engine ext-UI bundle not served — publish it first (build ce-wiresheet + ext ui)");
  }
});

test("the wiresheet renders its edge (wire) connections", async ({ page }) => {
  await page.goto(SHELL, { waitUntil: "networkidle" });
  await page.getByLabel("identity").fill("user:ada");
  await page.getByLabel("workspace").fill("acme");
  await page.getByLabel("sign in").click();

  // Open the Control Engine page from the real ext.list-built sidebar slot.
  await page.getByRole("button", { name: /Control Engine/i }).first().click();

  // The federated canvas mounts with the appliance's components...
  const nodes = page.locator(".react-flow__node");
  await expect(nodes.first()).toBeAttached({ timeout: 20_000 });

  // ...and the seeded random.out → dewpoint.rh edge MUST render as a wire. Before the fix
  // this was 0 (the promotion gate raced the grace drop and lost). We assert ≥1 edge path.
  const edgePaths = page.locator(".react-flow__edge-path");
  await expect(edgePaths.first()).toBeAttached({ timeout: 20_000 });
  expect(await edgePaths.count()).toBeGreaterThanOrEqual(1);
});
