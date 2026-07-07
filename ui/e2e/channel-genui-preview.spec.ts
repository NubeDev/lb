// E2E: the GenUI widget PREVIEW in a conversation (channel-widgets scope) — a `view:"genui"`
// rich_result posted through the REAL gateway (exactly the `channel.post` MCP call the agent makes)
// renders as the composed live surface in a real browser, NOT GenUiView's invalid/draft state.
//
// This is the deterministic guard for the live-model flow: the model's post either passes the host's
// genui gate (and MUST render), or is rejected loudly with a message that names the fix. Both halves
// are pinned here — a jsdom test can't prove the browser render, and a live-model run can't be a CI
// guard. Also pins the lenient-args normalization (a JSON-STRING `ir` lands as the parsed object —
// the exact shape the live model stalled on, 2026-07-06).
//
// Prereqs (asserted, with a clear message if absent): built shell on :4173 (`make ui-preview`) and a
// node on :8080 (`make dev`) — REBUILT + restarted so it carries the channel genui gate.

import { test, expect, type APIRequestContext } from "@playwright/test";

const SHELL = process.env.LB_SHELL_URL ?? "http://127.0.0.1:4173";
const GATEWAY = process.env.VITE_GATEWAY_URL ?? "http://127.0.0.1:8080";

// One-segment channel id (a dotted id is a multi-segment cap resource — the dock-channel-id lesson).
const CID = "e2e-genui-preview";

/** The exact IR the skill teaches: stack → text + slider + button + table bound to /data/A/rows. */
const IR = {
  v: 1,
  surface: { surfaceId: "e2e-s1", root: "root" },
  components: {
    root: { id: "root", component: "stack", props: {}, children: ["title", "limit", "run", "tbl"] },
    title: { id: "title", component: "text", props: { value: "GenUI e2e preview" } },
    limit: { id: "limit", component: "slider", props: { label: "limit", min: 1, max: 50, value: 10 } },
    run: { id: "run", component: "button", props: { label: "Run", value: "run" } },
    tbl: { id: "tbl", component: "table", props: { rows: { "$bind": "/data/A/rows" } } },
  },
};

function envelope(ir: unknown) {
  return JSON.stringify({
    kind: "rich_result",
    v: 2,
    view: "genui",
    options: { genui: { v: 1, ir } },
    sources: [
      {
        refId: "A",
        tool: "federation.query",
        args: { source: "demo-buildings", sql: "SELECT id, meter_id, name FROM point ORDER BY meter_id, id LIMIT 10" },
      },
    ],
    tools: ["federation.query"],
  });
}

/** `channel.post` over the bridge — byte-identical to the agent's tool call. */
async function postBody(request: APIRequestContext, token: string, id: string, body: string) {
  return request.post(`${GATEWAY}/mcp/call`, {
    headers: { authorization: `Bearer ${token}` },
    data: { tool: "channel.post", args: { cid: CID, id, ts: Date.now(), body } },
  });
}

let token = "";

test.beforeAll(async ({ request }) => {
  const shell = await request.get(SHELL).catch(() => null);
  if (!shell || !shell.ok()) {
    throw new Error(`built shell not reachable at ${SHELL} — run 'make ui-preview' first`);
  }
  const login = await request.post(`${GATEWAY}/login`, { data: { user: "user:ada", workspace: "acme" } });
  if (!login.ok()) throw new Error(`node not reachable / login failed at ${GATEWAY} — run 'make dev'`);
  token = (await login.json()).token as string;
});

test("the host gate rejects the wrong IR dialect with the fix named", async ({ request }) => {
  // The live defect verbatim: `type` for `component`, no ids, no v, no surface.
  const bad = envelope({ components: { root: { type: "stack" } } });
  const res = await postBody(request, token, "e2e-genui-bad", bad);
  expect(res.ok(), "a malformed genui rich_result must be rejected").toBe(false);
  const text = await res.text();
  expect(text).toContain("`v`"); // first named defect: missing numeric v
});

test("a JSON-string ir is normalized and lands as the parsed object", async ({ request }) => {
  const res = await postBody(request, token, "e2e-genui-str", envelope(JSON.stringify(IR)));
  expect(res.ok(), `stringified-but-valid ir must land: ${await res.text()}`).toBe(true);
  // Read it back over the same bridge the UI uses — the stored body must carry the OBJECT ir.
  const hist = await request.post(`${GATEWAY}/mcp/call`, {
    headers: { authorization: `Bearer ${token}` },
    data: { tool: "channel.history", args: { cid: CID } },
  });
  const messages = ((await hist.json())?.messages ?? []) as Array<{ id: string; body: string }>;
  const item = messages.find((m) => m.id === "e2e-genui-str");
  expect(item, "the normalized item is in history").toBeTruthy();
  const parsed = JSON.parse(item!.body);
  expect(typeof parsed.options.genui.ir).toBe("object");
  expect(parsed.options.genui.ir.surface.root).toBe("root");
});

test("a valid genui rich_result renders the composed surface in the channel", async ({ page, request }) => {
  const res = await postBody(request, token, "e2e-genui-good", envelope(IR));
  expect(res.ok(), `valid genui post must land: ${await res.text()}`).toBe(true);

  const fatal: string[] = [];
  page.on("pageerror", (e) => fatal.push(e.message));

  // Built shell → log in → Channels (default surface) → open the e2e channel from the rail.
  await page.goto(SHELL, { waitUntil: "networkidle" });
  await page.getByLabel("identity").fill("user:ada");
  await page.getByLabel("workspace").fill("acme");
  await page.getByLabel("sign in").click();
  await expect(page.getByRole("textbox", { name: "message" })).toBeVisible({ timeout: 15_000 });
  await page.getByRole("button", { name: CID }).click();

  // THE PREVIEW: the genui surface itself — not the draft ("author me") or invalid fallback.
  const surface = page.locator('[data-view="genui"]');
  await expect(surface.last()).toBeVisible({ timeout: 15_000 });
  // Both landed posts (the normalized string-ir one and this one) render — assert the last.
  await expect(page.getByText("GenUI e2e preview").last()).toBeVisible({ timeout: 15_000 });
  await expect(surface.last().locator(".gu-btn")).toBeVisible(); // the Run button
  await expect(surface.last().locator('input[type="range"]')).toBeVisible(); // the limit slider
  expect(fatal, `no fatal page errors: ${fatal.join("; ")}`).toHaveLength(0);

  await page.screenshot({ path: "e2e/__screenshots__/channel-genui-preview.png", fullPage: true });
});
