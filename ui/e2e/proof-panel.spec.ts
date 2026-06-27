// E2E: the `proof-panel` extension page loads and mounts in the BUILT shell (ui-federation scope).
//
// This is the regression guard for the federation rework — it replaces `@originjs/vite-plugin-federation`
// with the rubix-style import-map + externalised-React pattern. The chain of @originjs bugs (url-must-be-
// a-Promise, `__rf_placeholder__shareScope is not defined`, and finally "Invalid hook call — more than
// one copy of React") only manifest in a REAL browser loading the gateway-served remote, so a jsdom unit
// test cannot catch them. Here a real Chromium loads the production shell on :4173, logs in against the
// REAL node on :8080, opens the Proof Panel page, and asserts the remote actually mounted with the host's
// single React — no error wrapper, no hook-call crash, real content in the host element.

import { test, expect, type ConsoleMessage } from "@playwright/test";

const SHELL = process.env.LB_SHELL_URL ?? "http://127.0.0.1:4173";
const GATEWAY = process.env.VITE_GATEWAY_URL ?? "http://127.0.0.1:8080";

test.beforeAll(async ({ request }) => {
  // Fail fast with a clear message if the out-of-band servers (built shell + real node) aren't up.
  const shell = await request.get(SHELL).catch(() => null);
  if (!shell || !shell.ok()) {
    throw new Error(`built shell not reachable at ${SHELL} — run 'make ui-preview' first`);
  }
  const remote = await request
    .get(`${GATEWAY}/extensions/proof-panel/ui/remoteEntry.js`)
    .catch(() => null);
  if (!remote || !remote.ok()) {
    throw new Error(
      `proof-panel remote not served at ${GATEWAY} — run 'make publish-ext EXT=proof-panel' (node must be up)`,
    );
  }
});

test("proof-panel federated page mounts in the built shell with the host's single React", async ({
  page,
}) => {
  // Capture every console error and uncaught page error — the "Invalid hook call" surfaces as both.
  const consoleErrors: string[] = [];
  const pageErrors: string[] = [];
  page.on("console", (msg: ConsoleMessage) => {
    if (msg.type() === "error") consoleErrors.push(msg.text());
  });
  page.on("pageerror", (err) => pageErrors.push(err.message));

  // 1) Load the built shell → the login screen.
  await page.goto(SHELL, { waitUntil: "networkidle" });

  // 2) Log in as user:ada / acme against the real node (the form defaults to exactly these).
  await page.getByLabel("identity").fill("user:ada");
  await page.getByLabel("workspace").fill("acme");
  await page.getByLabel("sign in").click();

  // 3) The sidebar builds an "Proof Panel" slot from the real ext.list. Open it.
  const slot = page.getByRole("button", { name: "Proof Panel", exact: true });
  await expect(slot).toBeVisible({ timeout: 15_000 });
  await slot.click();

  // 4) The host element exists and the remote renders REAL content into it (the page title + the
  //    workspace badge proving the host ctx reached the remote). This only happens if `mount` ran —
  //    i.e. the remote bound to the host's React and did not crash on a hook call.
  const host = page.locator('[data-ext-host="proof-panel"]');
  await expect(host).toBeAttached({ timeout: 15_000 });
  await expect(host).toContainText("Proof Panel", { timeout: 15_000 });
  await expect(host).toContainText("acme"); // the workspace badge — host ctx reached the remote
  await expect(host.getByLabel("search series")).toBeVisible(); // the page's own interactive UI

  // 5) The all-features demo, live: the page CREATES the data it shows. Click "Write sample" →
  //    ingest.write stages + drains + series.latest reads it back → the committed value renders.
  await host.getByLabel("write sample").click();
  const demoLatest = host.getByTestId("demo-latest");
  await expect(demoLatest).toBeVisible({ timeout: 15_000 });
  await expect(demoLatest).toContainText(/value \d/); // a real committed value, not a placeholder

  // 6) Outbox status: Refresh re-reads outbox.status and the three lifecycle counts render.
  await host.getByLabel("refresh outbox").click();
  await expect(host.getByTestId("outbox-counts")).toBeVisible({ timeout: 15_000 });
  await expect(host.getByTestId("outbox-pending")).toContainText(/^\d+$/);

  // 7) Host-callback derive (host-callback scope): "Run derive" invokes the extension's OWN wasm tool
  //    `proof-panel.proof.derive`, which reads proof.demo (written in step 5) and writes
  //    proof.derived = value*2 — ALL through the host-mediated `host.call-tool` callback. The card
  //    shows the derived value, proving a wasm GUEST did real platform work end to end in the browser.
  await host.getByLabel("run derive").click();
  const deriveResult = host.getByTestId("derive-result");
  await expect(deriveResult).toBeVisible({ timeout: 15_000 });
  await expect(deriveResult).toContainText(/Derived \d/); // a real committed derived value
  // And the committed proof.derived series read back over the bridge.
  await expect(host.getByTestId("derived-latest")).toContainText(/value \d/, { timeout: 15_000 });

  // 8) The shell's honest error wrapper ("Could not load proof-panel: …") must NOT be present.
  await expect(page.getByText(/Could not load/i)).toHaveCount(0);

  // 9) Screenshot the mounted page for the session doc.
  await page.screenshot({ path: "e2e/__screenshots__/proof-panel-mounted.png", fullPage: true });

  // 7) No "Invalid hook call" / "more than one copy of React" and no uncaught errors at all.
  const allErrors = [...consoleErrors, ...pageErrors];
  const hookErrors = allErrors.filter(
    (e) => /Invalid hook call/i.test(e) || /more than one copy of React/i.test(e),
  );
  expect(hookErrors, `hook/two-React errors:\n${hookErrors.join("\n")}`).toEqual([]);
  expect(pageErrors, `uncaught page errors:\n${pageErrors.join("\n")}`).toEqual([]);
  expect(consoleErrors, `console errors:\n${consoleErrors.join("\n")}`).toEqual([]);
});
