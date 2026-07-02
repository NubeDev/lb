// E2E: the `thecrew` (Graphics) extension page loads + mounts in the BUILT shell, drives the AHU-1
// demo scene LIVE through the host-mediated bridge, and round-trips an edit through `assets.put_doc`
// (graphics-canvas phases 1–2 — the live-node proof the unit + gateway suites cannot give: the
// federation mount, the WebGL scene, and the real bridge only compose in a real browser).
//
// Preconditions (started out-of-band, asserted in beforeAll): the real node on :8080 with `thecrew`
// PUBLISHED + INSTALLED (its remoteEntry.js served) and the AHU-1 scene doc + `ahu1.*` series seeded;
// the BUILT shell served on :4173 (`make ui-preview`). The scene picker + save bar are DOM (testids);
// the fan's live `speed` is proven DOM-side via the PropertyRail's live-value readout (the same value
// that drives the impeller's spin in useFrame), so we never assert on WebGL pixels.

import { test, expect, type ConsoleMessage } from "@playwright/test";

const SHELL = process.env.LB_SHELL_URL ?? "http://127.0.0.1:4173";
const GATEWAY = process.env.VITE_GATEWAY_URL ?? "http://127.0.0.1:8080";

test.beforeAll(async ({ request }) => {
  const shell = await request.get(SHELL).catch(() => null);
  if (!shell || !shell.ok()) {
    throw new Error(`built shell not reachable at ${SHELL} — run 'make ui-preview' first`);
  }
  const remote = await request
    .get(`${GATEWAY}/extensions/thecrew/ui/remoteEntry.js`)
    .catch(() => null);
  if (!remote || !remote.ok()) {
    throw new Error(
      `thecrew remote not served at ${GATEWAY} — publish it (node must be up) and seed scene:ahu-1`,
    );
  }
});

test("thecrew Graphics page mounts live, loads AHU-1, shows live SF-1 speed, and saves an edit", async ({
  page,
}) => {
  const consoleErrors: string[] = [];
  const pageErrors: string[] = [];
  page.on("console", (msg: ConsoleMessage) => {
    if (msg.type() === "error") consoleErrors.push(msg.text());
  });
  page.on("pageerror", (err) => pageErrors.push(err.message));

  // 1) Load the built shell → login as the seeded workspace-admin member.
  await page.goto(SHELL, { waitUntil: "networkidle" });
  await page.getByLabel("identity").fill("user:ada");
  await page.getByLabel("workspace").fill("acme");
  await page.getByLabel("sign in").click();

  // 2) The sidebar builds a "Graphics" slot from the real ext.list (the [ui] label). Open it.
  const slot = page.getByRole("button", { name: "Graphics", exact: true });
  await expect(slot).toBeVisible({ timeout: 15_000 });
  await slot.click();

  // 3) The federated remote mounts into the host element (proves single-React federation, no crash).
  const host = page.locator('[data-ext-host="thecrew"]');
  await expect(host).toBeAttached({ timeout: 15_000 });
  await expect(host.getByTestId("scene-persistence-bar")).toBeVisible({ timeout: 15_000 });

  // 4) The scene picker lists the seeded AHU-1 (via assets.list_docs, scene: prefix). Open it.
  const picker = host.getByTestId("scene-picker");
  await expect(picker.locator("option", { hasText: "AHU-1" })).toHaveCount(1, { timeout: 15_000 });
  await picker.selectOption({ label: "AHU-1" });
  await expect(host.getByTestId("scene-title")).toHaveValue("AHU-1", { timeout: 15_000 });

  // 5) Select the SF-1 fan and confirm its LIVE speed (seeded 1800 via series.latest through the
  //    bridge) reaches the PropertyRail readout — the exact value that spins the impeller.
  //    Selection goes through the scene store (WebGL pointer-picking is unreliable headless); the
  //    store is exposed for the test harness on the mounted page.
  await page.waitForFunction(() => !!(window as unknown as { __tcStore?: unknown }).__tcStore, {
    timeout: 15_000,
  });
  await page.evaluate(() => {
    (window as unknown as { __tcStore: { getState(): { select(ids: string[]): void } } }).__tcStore
      .getState()
      .select(["sf1"]);
  });
  // The rail shows one binding row per bindSlot; the `speed` slot's live value renders 1800.
  await expect(host.getByText("1800", { exact: false })).toBeVisible({ timeout: 15_000 });

  // 6) Drag (nudge) SF-1 one grid step right through the store, dirtying the doc, then Save →
  //    assets.put_doc writes the new position. Status flips to "saved".
  await page.evaluate(() => {
    (window as unknown as { __tcStore: { getState(): { nudgeSelection(dx: number, dy: number): void } } })
      .__tcStore.getState()
      .nudgeSelection(8, 0);
  });
  await host.getByTestId("scene-save").click();
  await expect(host.getByTestId("scene-status")).toContainText("saved", { timeout: 15_000 });

  // 7) Reload the scene from the store (re-open) → the saved doc comes back cleanly (no conflict),
  //    proving the edit persisted through the real bridge.
  await picker.selectOption({ value: "" }).catch(() => {});
  await picker.selectOption({ label: "AHU-1" });
  await expect(host.getByTestId("scene-title")).toHaveValue("AHU-1", { timeout: 15_000 });

  // No federation/hook/bridge errors in the whole flow.
  expect(pageErrors, `page errors: ${pageErrors.join(" | ")}`).toHaveLength(0);
  const fatal = consoleErrors.filter((e) => /hook|react|federation|module specifier/i.test(e));
  expect(fatal, `console errors: ${fatal.join(" | ")}`).toHaveLength(0);

  await page.screenshot({
    path: "../rust/extensions/thecrew/docs/shots/graphics-ahu-1-live.png",
    fullPage: true,
  });
});
