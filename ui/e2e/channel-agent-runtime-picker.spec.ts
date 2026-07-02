// E2E: the in-channel `/agent` command MUST surface a runtime picker so a member can select a
// non-default runtime (e.g. `open-interpreter-default`) — the "no option to pick a runtime" bug.
//
// This is the guard the jsdom "gateway" suite could NOT be: the runtime dropdown only fails to appear
// in a REAL browser render of the palette (the arg-rail advance from the required `goal` to the OPTIONAL
// inline `runtime` widget). We drive the real built shell + real node: accept `/agent`, type a goal,
// commit it, and assert the runtime `<select>` renders AND lists the node's external runtimes.
//
// Prereqs (asserted, skipped-with-message if absent): built shell on :4173 (`make ui-preview`) and a
// node on :8080 started with the `external-agent` feature (`make dev EXTAGENT=1`) so `agent.runtimes`
// offers `open-interpreter-default`.

import { test, expect, type APIRequestContext } from "@playwright/test";

const SHELL = process.env.LB_SHELL_URL ?? "http://127.0.0.1:4173";
const GATEWAY = process.env.VITE_GATEWAY_URL ?? "http://127.0.0.1:8080";

/** The node's configured runtimes over the real read verb (so the spec asserts the SAME source the UI reads). */
async function nodeRuntimes(request: APIRequestContext, token: string): Promise<string[]> {
  const res = await request.post(`${GATEWAY}/mcp/call`, {
    headers: { authorization: `Bearer ${token}` },
    data: { tool: "agent.runtimes", args: {} },
  });
  const body = await res.json().catch(() => ({}));
  return (body?.runtimes ?? body?.result?.runtimes ?? []) as string[];
}

let runtimes: string[] = [];

test.beforeAll(async ({ request }) => {
  const shell = await request.get(SHELL).catch(() => null);
  if (!shell || !shell.ok()) {
    throw new Error(`built shell not reachable at ${SHELL} — run 'make ui-preview' first`);
  }
  const login = await request.post(`${GATEWAY}/login`, { data: { user: "user:ada", workspace: "acme" } });
  if (!login.ok()) throw new Error(`node not reachable / login failed at ${GATEWAY}`);
  const token = (await login.json()).token as string;
  runtimes = await nodeRuntimes(request, token);
  if (!runtimes.includes("open-interpreter-default")) {
    throw new Error(
      `node offers only ${JSON.stringify(runtimes)} — start it with 'make dev EXTAGENT=1' so an external runtime is selectable`,
    );
  }
});

test("the /agent command surfaces a runtime picker that lists the external runtimes", async ({ page }) => {
  // 1) Built shell → log in → land on Channels (the default surface).
  await page.goto(SHELL, { waitUntil: "networkidle" });
  await page.getByLabel("identity").fill("user:ada");
  await page.getByLabel("workspace").fill("acme");
  await page.getByLabel("sign in").click();

  // 2) The channel composer. Type `/` to open the command menu and pick the agent command.
  const message = page.getByRole("textbox", { name: "message" });
  await expect(message).toBeVisible({ timeout: 15_000 });
  await message.fill("/agent");
  await expect(page.getByRole("listbox", { name: "commands" })).toBeVisible({ timeout: 15_000 });
  await page.keyboard.press("Enter");

  // 3) THE FIX (the real UX): the goal field AND the runtime picker are BOTH visible the moment the
  //    command is picked — the runtime widget renders persistently, NOT gated behind committing the goal
  //    with a hidden ⏎ (the bug: the picker was the "next" active arg, so it never showed for a user who
  //    just typed a goal and pressed send).
  const goal = page.getByRole("textbox", { name: "goal" });
  await expect(goal).toBeVisible({ timeout: 15_000 });
  const runtime = page.getByRole("combobox", { name: "runtime" });
  await expect(runtime).toBeVisible({ timeout: 15_000 });

  // Typing the goal does NOT make the runtime picker disappear (both stay).
  await goal.fill("summarize the incident");
  await expect(runtime).toBeVisible();

  // 5) It lists the node's external runtimes and is user-selectable — pick open-interpreter-default.
  for (const id of runtimes) {
    await expect(runtime.locator("option", { hasText: id }).first()).toHaveCount(1, { timeout: 10_000 });
  }
  await runtime.selectOption("open-interpreter-default");
  await expect(runtime).toHaveValue("open-interpreter-default");

  await page.screenshot({ path: "e2e/__screenshots__/channel-agent-runtime-picker.png", fullPage: true });
});
