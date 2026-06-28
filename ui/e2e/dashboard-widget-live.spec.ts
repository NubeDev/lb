// E2E: the LIVE (SSE) extension widget — "Proof Ping Live" — added from the dashboard palette ticks on
// a new sample WITHOUT a reload (widget-builder scope "Live feed"; proof-panel's 2nd `[[widget]]`).
//
// This proves the whole motion chain end to end in a real browser: the in-process federated tile calls
// `bridge.watch("series.watch", {series:"proof.demo"})` → openSeriesStream → the gateway SSE
// `GET /series/proof.demo/stream` → the ws motion subject. We mount the tile, then write a fresh
// `proof.demo` sample over the real ingest path, and assert the tile's value updates to it live (the
// "live" badge turns on) — a jsdom unit test can't exercise a real EventSource against a real gateway.

import { test, expect, type ConsoleMessage } from "@playwright/test";

const SHELL = process.env.LB_SHELL_URL ?? "http://127.0.0.1:4173";
const GATEWAY = process.env.VITE_GATEWAY_URL ?? "http://127.0.0.1:8080";

type Api = import("@playwright/test").APIRequestContext;

/** Write one `proof.demo` sample through the real ingest path (publishes motion the SSE streams). */
async function writeSample(request: Api, token: string, seq: number, payload: number) {
  await request.post(`${GATEWAY}/ingest`, {
    headers: { authorization: `Bearer ${token}` },
    data: {
      samples: [
        { series: "proof.demo", producer: "e2e-live", ts: seq, seq, payload, labels: null, qos: "best-effort" },
      ],
    },
  });
}

/** The current `series.latest` seq for proof.demo (0 if none) — so we always write ABOVE it and the
 *  tile's backfill/`latest` reflects OUR sample, regardless of the workspace's prior history. */
async function latestSeq(request: Api, token: string): Promise<number> {
  const res = await request.post(`${GATEWAY}/mcp/call`, {
    headers: { authorization: `Bearer ${token}` },
    data: { tool: "series.latest", args: { series: "proof.demo" } },
  });
  const body = await res.json().catch(() => ({}));
  const seq = body?.sample?.seq ?? body?.result?.sample?.seq;
  return typeof seq === "number" ? seq : 0;
}

let token = "";
let baseSeq = 0;

test.beforeAll(async ({ request }) => {
  const shell = await request.get(SHELL).catch(() => null);
  if (!shell || !shell.ok()) {
    throw new Error(`built shell not reachable at ${SHELL} — run 'make ui-preview' first`);
  }
  const remote = await request.get(`${GATEWAY}/extensions/proof-panel/ui/remoteEntry.js`).catch(() => null);
  if (!remote || !remote.ok()) {
    throw new Error(`proof-panel remote not served — run 'make publish-ext EXT=proof-panel'`);
  }
  const login = await request.post(`${GATEWAY}/login`, { data: { user: "user:ada", workspace: "acme" } });
  token = (await login.json()).token as string;
  // Establish a backfill value ABOVE any prior history so the tile's latest is deterministically ours.
  baseSeq = (await latestSeq(request, token)) + 1;
  await writeSample(request, token, baseSeq, 11);
});

test("a live tile added from the palette ticks on a new sample with no reload", async ({
  page,
  request,
}) => {
  const fatal: string[] = [];
  page.on("console", (m: ConsoleMessage) => {
    if (m.type() === "error" && /Invalid hook call|more than one copy of React|Failed to resolve module specifier/i.test(m.text()))
      fatal.push(m.text());
  });
  page.on("pageerror", (e) => fatal.push(e.message));

  // 1) Built shell → log in → Dashboards → create a fresh dashboard.
  await page.goto(SHELL, { waitUntil: "networkidle" });
  await page.getByLabel("identity").fill("user:ada");
  await page.getByLabel("workspace").fill("acme");
  await page.getByLabel("sign in").click();
  await page.getByRole("button", { name: "Dashboards", exact: true }).click();
  const titleInput = page.getByLabel("new dashboard title");
  await expect(titleInput).toBeVisible({ timeout: 15_000 });
  await titleInput.fill("Live E2E");
  await page.getByLabel("create dashboard").click();

  // 2) Pick the LIVE tile from the "Extension widgets" palette group (the 2nd [[widget]]).
  const source = page.getByLabel("widget source");
  await expect(source).toBeVisible({ timeout: 15_000 });
  await expect(
    source.locator("option").filter({ hasText: /^proof-panel · Proof Ping Live$/ }),
  ).toHaveCount(1, { timeout: 15_000 });
  await source.selectOption({ label: "proof-panel · Proof Ping Live" });

  // 3) The preview mounts the live tile in-process; it backfills the latest value (our seeded 11).
  const preview = page.locator('[data-ext-widget="proof-panel"][data-tier="in-process"]').first();
  await expect(preview).toBeAttached({ timeout: 15_000 });
  await expect(preview.locator("[data-proof-live-widget]")).toBeVisible({ timeout: 15_000 });
  await expect(preview.getByLabel("proof live widget value")).toHaveText("11", { timeout: 15_000 });

  // 4) THE LIVE PROOF: write a NEW sample (higher seq) over the real ingest path. The SSE delivers it
  //    and the tile updates to the new value WITHOUT a reload, and the badge flips to "live".
  await writeSample(request, token, baseSeq + 1, 73);
  await expect(preview.getByLabel("proof live widget value")).toHaveText("73", { timeout: 15_000 });
  await expect(preview.locator('[data-live="on"]')).toBeVisible({ timeout: 15_000 });

  // 5) A second live sample also folds in — motion, not a one-shot.
  await writeSample(request, token, baseSeq + 2, 88);
  await expect(preview.getByLabel("proof live widget value")).toHaveText("88", { timeout: 15_000 });

  await page.screenshot({ path: "e2e/__screenshots__/dashboard-widget-live.png", fullPage: true });
  expect(fatal, `fatal federation errors:\n${fatal.join("\n")}`).toHaveLength(0);
});
