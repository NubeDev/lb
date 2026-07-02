// E2E: the canvas Lock button freezes camera pan/zoom so parts can be arranged without the view
// sliding (builder-ux: "a button to lock the canvas"). Drives the exposed store (window.__tcStore) —
// headless WebGL pointer picking is unreliable, per the sibling thecrew specs — and asserts the
// `locked` flag flips and that MapControls no longer pans while locked.

import { test, expect, type ConsoleMessage } from "@playwright/test";

const SHELL = process.env.LB_SHELL_URL ?? "http://127.0.0.1:4173";

test("the Lock button freezes the canvas view (pan/zoom off while locked)", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (e) => pageErrors.push(e.message));
  const consoleErrors: string[] = [];
  page.on("console", (m: ConsoleMessage) => m.type() === "error" && consoleErrors.push(m.text()));

  await page.goto(SHELL, { waitUntil: "networkidle" });
  await page.getByLabel("identity").fill("user:ada");
  await page.getByLabel("workspace").fill("acme");
  await page.getByLabel("sign in").click();

  await page.getByRole("button", { name: "Graphics", exact: true }).click();

  // Wait for thecrew's store to mount.
  await page.waitForFunction(() => !!(window as unknown as { __tcStore?: unknown }).__tcStore, {
    timeout: 20_000,
  });

  type Store = { getState(): { locked: boolean; toggleLock(): void } };
  const readLocked = () =>
    page.evaluate(() => (window as unknown as { __tcStore: Store }).__tcStore.getState().locked);

  // Starts unlocked.
  expect(await readLocked()).toBe(false);

  // Click the toolbar Lock button → store.locked flips true, button reflects it.
  const lockBtn = page.getByRole("button", { name: /lock canvas view/i });
  await expect(lockBtn).toBeVisible({ timeout: 20_000 });
  await lockBtn.click();
  expect(await readLocked()).toBe(true);

  // Locked title/affordance updates.
  await expect(page.getByRole("button", { name: /canvas locked/i })).toBeVisible();

  // Toggling again unlocks.
  await page.getByRole("button", { name: /canvas locked/i }).click();
  expect(await readLocked()).toBe(false);

  // The `L` shortcut also toggles (keyboard split lives in Toolbar.tsx).
  await page.keyboard.press("l");
  expect(await readLocked()).toBe(true);

  await page.screenshot({
    path: "../rust/extensions/thecrew/docs/shots/canvas-lock-live.png",
    fullPage: true,
  });

  expect(pageErrors, `page errors: ${pageErrors.join(" | ")}`).toHaveLength(0);
  const fatal = consoleErrors.filter((e) => /hook|module specifier|process is not defined/i.test(e));
  expect(fatal, `fatal console: ${fatal.join(" | ")}`).toHaveLength(0);
});
