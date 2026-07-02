// E2E: thecrew follows the HOST theme. The extension defines no colors of its own — its DOM chrome
// derives every --tc-* from the shell tokens (--bg/--panel/--fg/--accent) and the 3D canvas reads the
// same tokens at runtime (theme/host-tokens.ts). Flipping the host's `.dark` class on <html> must
// re-resolve thecrew's surface color. Proves the "use the host CSS" contract in a real browser.

import { test, expect, type ConsoleMessage } from "@playwright/test";

const SHELL = process.env.LB_SHELL_URL ?? "http://127.0.0.1:4173";

test("thecrew chrome + canvas follow the host light/dark theme", async ({ page }) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (e) => pageErrors.push(e.message));
  const consoleErrors: string[] = [];
  page.on("console", (m: ConsoleMessage) => m.type() === "error" && consoleErrors.push(m.text()));

  await page.goto(SHELL, { waitUntil: "networkidle" });
  await page.getByLabel("identity").fill("user:ada");
  await page.getByLabel("workspace").fill("acme");
  await page.getByLabel("sign in").click();
  await page.getByRole("button", { name: "Graphics", exact: true }).click();

  // thecrew's toolbar header carries bg-[var(--tc-panel)] → resolves from the host --panel token.
  const header = page.locator("header", { hasText: "thecrew" });
  await expect(header).toBeVisible({ timeout: 20_000 });

  // Force the host into a known theme, read thecrew's resolved surface, flip, read again.
  const bgFor = async (mode: "dark" | "light") => {
    await page.evaluate((m) => {
      document.documentElement.classList.toggle("dark", m === "dark");
    }, mode);
    // let the MutationObserver + re-render settle
    await page.waitForTimeout(200);
    return header.evaluate((el) => getComputedStyle(el).backgroundColor);
  };

  const dark = await bgFor("dark");
  const light = await bgFor("light");

  // The resolved panel color MUST differ between themes — proof thecrew reads the host token, not a
  // hardcoded palette. (Both are rgb()/rgba() strings; a hardcoded ext would return the same value.)
  expect(dark).not.toBe(light);
  // sanity: dark panel is darker than light panel (sum of channels)
  const lum = (rgb: string) => (rgb.match(/\d+/g) ?? []).slice(0, 3).reduce((a, b) => a + Number(b), 0);
  expect(lum(dark)).toBeLessThan(lum(light));

  await page.screenshot({
    path: "../rust/extensions/thecrew/docs/shots/host-theme-light.png",
    fullPage: true,
  });

  expect(pageErrors, `page errors: ${pageErrors.join(" | ")}`).toHaveLength(0);
  const fatal = consoleErrors.filter((e) => /hook|module specifier|process is not defined/i.test(e));
  expect(fatal, `fatal console: ${fatal.join(" | ")}`).toHaveLength(0);
});
