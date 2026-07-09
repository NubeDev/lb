import { test, expect } from "@playwright/test";

const SHELL = "http://127.0.0.1:5173";

test("shoot sidebar", async ({ page }) => {
  await page.goto(SHELL, { waitUntil: "networkidle" });
  await page.getByLabel("identity").fill("user:ada");
  await page.getByLabel("workspace").fill("acme");
  await page.getByLabel("sign in").click();
  // wait for rail
  await expect(page.getByRole("button", { name: "Dashboards", exact: true })).toBeVisible({ timeout: 20000 });
  await page.waitForTimeout(1200);
  await page.screenshot({ path: "e2e/__screenshots__/_sidebar-current.png", fullPage: false });
  // also grab just the sidebar element if present
  const rail = page.locator('[data-sidebar="sidebar"]').first();
  if (await rail.count()) {
    await rail.screenshot({ path: "e2e/__screenshots__/_sidebar-only.png" });
  }
});
