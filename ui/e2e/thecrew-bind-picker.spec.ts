// E2E: the scene property rail binds a shape prop through the reusable @nube/source-picker, LIVE
// (source-picker-package-scope.md, thecrew consumer). Proves the whole wired path in a real browser:
// the bridge-backed SourceLoaders → `series.list` over the page bridge → the picker lists EVERY
// workspace series (not just already-bound channels) → picking one sets the shape's bind channel.

import { test, expect, type ConsoleMessage } from "@playwright/test";

const SHELL = process.env.LB_SHELL_URL ?? "http://127.0.0.1:4173";

test("a scene shape binds a prop through the reusable source picker (series discovered over the bridge)", async ({
  page,
}) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (e) => pageErrors.push(e.message));
  const consoleErrors: string[] = [];
  page.on("console", (m: ConsoleMessage) => m.type() === "error" && consoleErrors.push(m.text()));

  await page.goto(SHELL, { waitUntil: "networkidle" });
  await page.getByLabel("identity").fill("user:ada");
  await page.getByLabel("workspace").fill("acme");
  await page.getByLabel("sign in").click();

  // Open the Graphics page + the AHU-1 scene.
  await page.getByRole("button", { name: "Graphics", exact: true }).click();
  await page.getByTestId("scene-picker").selectOption({ label: "AHU-1" });

  // Select SF-1 deterministically via the exposed store (headless WebGL picking is unreliable).
  await page.waitForFunction(() => !!(window as unknown as { __tcStore?: unknown }).__tcStore, {
    timeout: 20_000,
  });
  await page.evaluate(() => {
    (window as unknown as { __tcStore: { getState(): { select(ids: string[]): void } } }).__tcStore
      .getState()
      .select(["sf1"]);
  });

  // The rail shows SF-1's bind slots, each a reusable source picker. The `speed` slot's picker must
  // list workspace series discovered over the bridge via `series.list` — including one NOT bound in the
  // scene (proving discovery, not the old already-bound-only loop).
  const speed = page.locator('select[aria-label="bind speed"]');
  await expect(speed).toBeVisible({ timeout: 20_000 });
  // The picker lists workspace series discovered over the bridge — including ones NOT bound to `speed`,
  // proving discovery (not the old already-bound-only loop). Exact-text options in the Series group.
  await expect(speed.getByRole("option", { name: "ahu1.sf1.speed", exact: true })).toHaveCount(1);
  await expect(speed.getByRole("option", { name: "ahu1.oad.position", exact: true })).toHaveCount(1);

  // Re-bind speed to a different series through the picker → the store's bind channel updates.
  await speed.selectOption({ label: "ahu1.rat" });
  const boundChannel = await page.evaluate(() => {
    const s = (window as unknown as { __tcStore: { getState(): { doc: { shapes: Record<string, { bind?: Record<string, { channel: string }> }> } } } }).__tcStore;
    return s.getState().doc.shapes["sf1"]?.bind?.speed?.channel;
  });
  expect(boundChannel).toBe("ahu1.rat");

  await page.screenshot({ path: "../rust/extensions/thecrew/docs/shots/scene-bind-picker-live.png", fullPage: true });

  expect(pageErrors, `page errors: ${pageErrors.join(" | ")}`).toHaveLength(0);
  const fatal = consoleErrors.filter((e) => /hook|module specifier|process is not defined/i.test(e));
  expect(fatal, `fatal console: ${fatal.join(" | ")}`).toHaveLength(0);
});
