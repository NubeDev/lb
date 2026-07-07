// E2E: the GenUI widget preview in the AGENT DOCK — the surface the user actually tests in (the
// channel-surface twin is `channel-genui-preview.spec.ts`; this one exists because "it renders in
// Channels" was proven while the user saw nothing in the DOCK). A dock session is an ordinary
// channel (`dock-{user-slug}-{ulid}`, dockId.ts) rendered through the dock's own mount of
// MessageList → MessageItem → ResponseView → WidgetView → GenUiView; this spec posts the same
// genui rich_result the agent posts (real gateway, `channel.post`) into a dock-prefixed session
// and asserts the composed surface renders INSIDE the dock panel.
//
// Prereqs: built shell on :4173 (`make ui-preview`), node on :8080 (`make dev`).

import { test, expect } from "@playwright/test";

const SHELL = process.env.LB_SHELL_URL ?? "http://127.0.0.1:4173";
const GATEWAY = process.env.VITE_GATEWAY_URL ?? "http://127.0.0.1:8080";

// A session id matching user:ada's dock prefix (`dock-` + userSlug("user:ada") + `-…`), ONE cap
// segment. A fixed suffix: re-runs replace the same messages (idempotent post ids).
const CID = "dock-user-ada-e2egenui01";

const ENVELOPE = JSON.stringify({
  kind: "rich_result",
  v: 2,
  view: "genui",
  options: {
    genui: {
      v: 1,
      ir: {
        v: 1,
        surface: { surfaceId: "dock-e2e-s1", root: "root" },
        components: {
          root: { id: "root", component: "stack", props: {}, children: ["title", "tbl"] },
          title: { id: "title", component: "text", props: { value: "Dock GenUI e2e preview" } },
          tbl: { id: "tbl", component: "table", props: { rows: { "$bind": "/data/A/rows" } } },
        },
      },
    },
  },
  sources: [
    {
      refId: "A",
      tool: "federation.query",
      args: { source: "demo-buildings", sql: "SELECT id, name FROM site LIMIT 5" },
    },
  ],
  tools: ["federation.query"],
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
  const ask = await post("e2e-dock-ask", "make me an example widget using genui");
  if (!ask.ok()) throw new Error(`seeding the dock session failed: ${await ask.text()}`);
  const widget = await post("e2e-dock-widget", ENVELOPE);
  if (!widget.ok()) throw new Error(`the genui post was rejected: ${await widget.text()}`);
});

test("a posted genui rich_result renders the composed surface inside the agent dock", async ({ page }) => {
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

  // THE PREVIEW, in the dock panel: the genui surface — not raw JSON, not the draft/invalid state.
  await expect(page.locator('[data-view="genui"]').last()).toBeVisible({ timeout: 15_000 });
  await expect(page.getByText("Dock GenUI e2e preview").last()).toBeVisible({ timeout: 15_000 });
  expect(fatal, `no fatal page errors: ${fatal.join("; ")}`).toHaveLength(0);

  await page.screenshot({ path: "e2e/__screenshots__/agent-dock-genui-preview.png", fullPage: true });
});
