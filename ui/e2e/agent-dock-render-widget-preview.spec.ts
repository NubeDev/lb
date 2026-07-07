// E2E: the built-in render-widget preview in the AGENT DOCK — the non-genui twin of
// agent-dock-genui-preview.spec.ts. The dock's no-`channel.post` widget pipeline is GENERIC over
// the `rich_result` view: the same MessageItem → ResponseView → WidgetView path that renders a
// `view:"genui"` envelope also renders a `view:"stat"` / `"chart"` / `"gauge"` / `"table"` one. This
// spec posts a `view:"stat"` rich_result (the shipped StatPanel, bound to a real store.query) into a
// dock-prefixed session channel and asserts the rendered stat appears INSIDE the dock panel — locking
// the built-in render path end-to-end.
//
// Prereqs: built shell on :4173 (`make ui-preview`), node on :8080 (`make dev`).

import { test, expect } from "@playwright/test";

const SHELL = process.env.LB_SHELL_URL ?? "http://127.0.0.1:4173";
const GATEWAY = process.env.VITE_GATEWAY_URL ?? "http://127.0.0.1:8080";

// A session id matching user:ada's dock prefix (`dock-` + userSlug("user:ada") + `-…`), ONE cap
// segment. A fixed suffix: re-runs replace the same messages (idempotent post ids).
const CID = "dock-user-ada-e2estat01";

// A built-in `view:"stat"` envelope (NOT genui) — the shipped StatPanel renderer bound to a real
// store.query over synthetic rows. The host re-runs the source at view time; `fieldConfig.defaults.unit`
// styles the value as seconds (the render-widgets skill's "average session time" example).
const ENVELOPE = JSON.stringify({
  kind: "rich_result",
  v: 2,
  view: "stat",
  source: {
    tool: "store.query",
    args: {
      // SurrealQL inline-row source — a synthetic-but-real row (no table needed, dashboard-mcp §4).
      sql: "SELECT * FROM [{ value: 1842 }]",
    },
  },
  options: {
    reduceOptions: { calcs: ["last"], fields: ["value"] },
    textMode: "auto",
    colorMode: "value",
  },
  fieldConfig: { defaults: { unit: "s" } },
  tools: ["store.query"],
});

let token = "";

test.beforeAll(async ({ request }) => {
  const shell = await request.get(SHELL).catch(() => null);
  if (!shell || !shell.ok()) {
    throw new Error(`built shell not reachable at ${SHELL} — run 'make ui-preview' first`);
  }
  const login = await request.post(`${GATEWAY}/login`, { data: { user: "user:ada", workspace: "acme" } });
  if (!login.ok()) throw new Error(`node not reachable / login failed at ${GATEWAY} — run 'make dev'`);
  token = (await login.json()).token as string;

  // Seed the dock session exactly as a run does: the user's ask, then the agent's widget post.
  const post = (id: string, body: string) =>
    request.post(`${GATEWAY}/mcp/call`, {
      headers: { authorization: `Bearer ${token}` },
      data: { tool: "channel.post", args: { cid: CID, id, ts: Date.now(), body } },
    });
  const ask = await post("e2e-dock-ask", "make me a stat widget for avg session time");
  if (!ask.ok()) throw new Error(`seeding the dock session failed: ${await ask.text()}`);
  const widget = await post("e2e-dock-widget", ENVELOPE);
  if (!widget.ok()) throw new Error(`the stat post was rejected: ${await widget.text()}`);
});

test("a posted stat rich_result renders the built-in widget inside the agent dock", async ({ page }) => {
  const fatal: string[] = [];
  page.on("pageerror", (e) => fatal.push(e.message));

  await page.goto(SHELL, { waitUntil: "networkidle" });
  await page.getByLabel("identity").fill("user:ada");
  await page.getByLabel("workspace").fill("acme");
  await page.getByLabel("sign in").click();
  await expect(page.getByRole("textbox", { name: "message" })).toBeVisible({ timeout: 15_000 });

  // Open the dock and switch to the seeded session (the picker lists the user's own dock-… ids).
  await page.getByLabel("toggle agent dock").click();
  const picker = page.getByLabel("dock session");
  await expect(picker).toBeVisible({ timeout: 15_000 });
  await picker.selectOption(CID);

  // THE PREVIEW, in the dock panel: the StatPanel — not raw JSON, not the denied/empty state. The
  // shipped StatPanel renders `aria-label="stat panel"` + `aria-label="stat value"`.
  await expect(page.getByLabel("stat panel").last()).toBeVisible({ timeout: 15_000 });
  // The synthetic source row {value: 1842} reduces to "1842" (the last/only value); the unit bridge
  // formats seconds, but the digit prefix is stable regardless of unit suffix.
  await expect(page.getByLabel("stat value").last()).toContainText("1842", { timeout: 15_000 });
  expect(fatal, `no fatal page errors: ${fatal.join("; ")}`).toHaveLength(0);

  await page.screenshot({ path: "e2e/__screenshots__/agent-dock-render-widget-preview.png", fullPage: true });
});
