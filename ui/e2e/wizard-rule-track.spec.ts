// E2E (verify pass, panel-wizard-source-discoverability slice 2): the new-panel wizard's RULE track
// now has parity with the datasource track — pick a saved rule under "Workspace source" → a Run button
// appears → running it shows the rule's rows, so the rule is PROVEN before binding. Drives the live shell
// in a real browser against the real node (the confusion + fix only exist in the rendered wizard).

import { test, expect } from "@playwright/test";

const SHELL = process.env.LB_SHELL_URL ?? "http://127.0.0.1:5173";
const GATEWAY = process.env.VITE_GATEWAY_URL ?? "http://127.0.0.1:8080";

test.beforeAll(async ({ request }) => {
  const shell = await request.get(SHELL).catch(() => null);
  if (!shell || !shell.ok()) throw new Error(`shell not reachable at ${SHELL}`);
  // Seed a saved rule through the real MCP path so the picker's Rules group is non-empty.
  const login = await request.post(`${GATEWAY}/login`, {
    data: { user: "user:ada", workspace: "acme" },
  });
  const token = (await login.json()).token as string;
  await request.post(`${GATEWAY}/mcp/call`, {
    headers: { authorization: `Bearer ${token}` },
    data: {
      tool: "rules.save",
      args: {
        id: "e2e-demo",
        name: "E2E Demo Rule",
        body: "let rows = [#{ h: 0, v: 10 }, #{ h: 1, v: 20 }]; rows",
      },
    },
  });
});

test("the wizard rule track shows Run + result so a rule is proven before binding", async ({ page }) => {
  await page.goto(SHELL, { waitUntil: "networkidle" });
  await page.getByLabel("identity").fill("user:ada");
  await page.getByLabel("workspace").fill("acme");
  await page.getByLabel("sign in").click();

  await page.getByRole("button", { name: "Dashboards", exact: true }).click();
  const titleInput = page.getByLabel("new dashboard title");
  await expect(titleInput).toBeVisible({ timeout: 15_000 });
  await titleInput.fill("Rule Wizard E2E");
  await page.getByLabel("create dashboard").click();

  // Open the new-panel wizard.
  await page.getByRole("button", { name: /new panel/i }).click();
  await expect(page.getByLabel("wizard source step")).toBeVisible({ timeout: 15_000 });

  // Card scent: the Workspace-source card names "rule".
  const wsCard = page.getByLabel("source track workspace");
  await expect(wsCard).toContainText(/rule/i);

  // Click 1 — the Workspace-source card. Click 2 — open the source combobox.
  await wsCard.click();
  await page.getByRole("combobox", { name: "wizard source" }).click();

  // The Rules group leads the list; pick the seeded rule.
  await page.getByRole("option", { name: "E2E Demo Rule" }).click();

  // The prove-it workbench appears with a Run button (parity with the datasource track).
  const run = page.getByLabel("run rule");
  await expect(run).toBeVisible({ timeout: 15_000 });
  await run.click();

  // Running shows the rule's returned rows — the scalar/JSON result carrying v:10 and v:20.
  const result = page.getByLabel("run result");
  await expect(result).toBeVisible({ timeout: 15_000 });
  await expect(result).toContainText("10");
  await expect(result).toContainText("20");

  await page.screenshot({ path: "e2e/__screenshots__/wizard-rule-track.png", fullPage: false });
});
