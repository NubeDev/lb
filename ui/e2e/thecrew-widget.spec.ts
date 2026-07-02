// E2E: the `thecrew` read-only Scene [[widget]] rendered LIVE in a dashboard cell (graphics-canvas
// phases 1–2). Proves the widget tier: the packaged tile mounts in-process, renders the AHU-1 scene +
// live values over the NARROWER widget bridge, and exposes NO save bar (read-only — the widget's
// manifest scope omits assets.put_doc/list_docs).
//
// We DRIVE THE BUILDER PALETTE (finding 7, now fixed): the restored "Extension widgets" PickerGroup in
// the PanelEditor Query tab makes the packaged `[[widget]]` pickable again, and the Scene tile surfaces
// a scene picker (finding 8) that sets `cell.options.sceneId`. This test adds the tile THROUGH THE UI —
// Add panel → pick "thecrew · Scene" → pick the AHU-1 scene → Save — instead of seeding the cell, so the
// whole author path is exercised end to end. It then asserts the same read-only render contract.
//
// Preconditions (out-of-band): the real node on :8080 with `thecrew` published, the AHU-1 scene + its
// `ahu1.*` series seeded, and a dashboard the member may edit; the built shell on :4173.

import { test, expect, type ConsoleMessage } from "@playwright/test";

const SHELL = process.env.LB_SHELL_URL ?? "http://127.0.0.1:4173";
const GATEWAY = process.env.VITE_GATEWAY_URL ?? "http://127.0.0.1:8080";

test.beforeAll(async ({ request }) => {
  const shell = await request.get(SHELL).catch(() => null);
  if (!shell || !shell.ok()) throw new Error(`built shell not reachable at ${SHELL}`);
  const remote = await request
    .get(`${GATEWAY}/extensions/thecrew/ui/remoteEntry.js`)
    .catch(() => null);
  if (!remote || !remote.ok()) throw new Error(`thecrew remote not served at ${GATEWAY}`);
});

async function signIn(page: import("@playwright/test").Page) {
  await page.goto(SHELL, { waitUntil: "networkidle" });
  await page.getByLabel("identity").fill("user:ada");
  await page.getByLabel("workspace").fill("acme");
  await page.getByLabel("sign in").click();
  await page.getByRole("button", { name: "Dashboards", exact: true }).click();
}

test("a Scene tile can be added THROUGH the builder palette (findings 7+8), then renders read-only", async ({
  page,
}) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (err) => pageErrors.push(err.message));
  const consoleErrors: string[] = [];
  page.on("console", (m: ConsoleMessage) => m.type() === "error" && consoleErrors.push(m.text()));

  await signIn(page);
  // The EMPTY editable dashboard — we build the cell here through the UI (not a seeded cell).
  await page.getByRole("button", { name: /select dashboard scene-build/i }).click();

  // --- drive the restored palette (finding 7): Add panel -> pick the packaged Scene tile. ---
  await page.getByLabel("add panel").click();
  // The "Extension widgets" group is restored in the Query tab's source picker; pick the Scene tile by
  // its friendly label (the option value is the widget entry id).
  await page.getByLabel("panel source").selectOption({ label: "thecrew · Scene" });
  // --- finding 8: the Scene picker (over assets.list_docs `scene:` docs) sets cell.options.sceneId. ---
  await page.getByLabel("scene doc").selectOption({ label: "AHU-1" });
  await page.getByLabel("save panel").click();

  // The ext widget mounts in-process (the publish/install cap is the trust gate — installed ext
  // widgets always federate in-process, never an iframe).
  const cell = page.locator('[data-ext-widget="thecrew"][data-tier="in-process"]').first();
  await expect(cell).toBeAttached({ timeout: 20_000 });

  // The scene rendered: the read-only cell mounts <SceneCanvas> (a three.js <canvas>), NOT the empty
  // "scene unavailable" state — proving the sceneId reached the widget (via ctx.options.sceneId, wired
  // by ExtWidget in finding 8) and the doc loaded over the bridge.
  await expect(cell.locator("canvas").first()).toBeAttached({ timeout: 20_000 });
  await expect(cell.getByTestId("scene-widget-empty")).toHaveCount(0);
  // The doc loaded (the `ready` gate) — `scene-widget` only mounts once loadScene resolved over the
  // bridge, so this proves the scene reached the widget, not just an empty canvas.
  await expect(cell.getByTestId("scene-widget")).toBeVisible({ timeout: 20_000 });

  // Read-only: the cell must NOT carry the page's persistence bar or Save button.
  await expect(cell.getByTestId("scene-persistence-bar")).toHaveCount(0);
  await expect(cell.getByTestId("scene-save")).toHaveCount(0);

  // Let the WebGL scene paint + the ortho camera auto-fit (FitCamera) settle before the shot so the
  // screenshot captures the framed scene (the "fit per cell" fix), not a mid-mount blank frame.
  await cell.locator("canvas").first().waitFor({ state: "visible" });
  await page.waitForTimeout(2000);

  await page.screenshot({
    path: "../rust/extensions/thecrew/docs/shots/scene-widget-dashboard.png",
    fullPage: true,
  });

  expect(pageErrors, `page errors: ${pageErrors.join(" | ")}`).toHaveLength(0);
  await expect(page.getByText(/could not load/i)).toHaveCount(0);
  const fatal = consoleErrors.filter((e) => /hook|module specifier|process is not defined/i.test(e));
  expect(fatal, `fatal console: ${fatal.join(" | ")}`).toHaveLength(0);
});
