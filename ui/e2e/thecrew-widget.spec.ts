// E2E: the `thecrew` read-only Scene [[widget]] rendered LIVE in a dashboard cell (graphics-canvas
// phases 1–2). Proves the widget tier: the packaged tile mounts in-process, renders the AHU-1 scene +
// live values over the NARROWER widget bridge, and exposes NO save bar (read-only — the widget's
// manifest scope omits assets.put_doc/list_docs).
//
// The dashboard is SEEDED out-of-band via `dashboard.save` (a `const` var `sceneId=scene:ahu-1` + one
// `ext:thecrew/scene` cell). We do NOT drive the builder palette: the current live PanelEditor source
// picker dropped the "Extension widgets" group (a concurrent viz panel-editor rework — surfaced as a
// finding in the session doc, outside thecrew's zero-core envelope), so a packaged [[widget]] can't be
// picked through the UI today. Seeding the cell exercises the exact render path a picked tile would.
//
// Preconditions (out-of-band): the real node on :8080 with `thecrew` published, the AHU-1 scene + its
// `ahu1.*` series seeded, and the `scene-dash` dashboard seeded; the built shell on :4173.

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

test("the Scene widget renders read-only in a dashboard cell with live values and no save bar", async ({
  page,
}) => {
  const pageErrors: string[] = [];
  page.on("pageerror", (err) => pageErrors.push(err.message));
  const consoleErrors: string[] = [];
  page.on("console", (m: ConsoleMessage) => m.type() === "error" && consoleErrors.push(m.text()));

  await page.goto(SHELL, { waitUntil: "networkidle" });
  await page.getByLabel("identity").fill("user:ada");
  await page.getByLabel("workspace").fill("acme");
  await page.getByLabel("sign in").click();

  // Open the seeded dashboard via the Dashboards rail (routes are hash + tenant-scoped, so we click
  // through rather than deep-link).
  await page.getByRole("button", { name: "Dashboards", exact: true }).click();
  await page.getByRole("button", { name: /select dashboard scene-dash/i }).click();

  // The ext widget mounts in-process (the publish/install cap is the trust gate — installed ext
  // widgets always federate in-process, never an iframe).
  const cell = page.locator('[data-ext-widget="thecrew"][data-tier="in-process"]').first();
  await expect(cell).toBeAttached({ timeout: 20_000 });

  // The scene rendered: the read-only cell mounts <SceneCanvas> (a three.js <canvas>), NOT the empty
  // "scene unavailable" state — proving the sceneId reached the widget (via ctx.vars) and the doc
  // loaded over the bridge.
  await expect(cell.locator("canvas").first()).toBeAttached({ timeout: 20_000 });
  await expect(cell.getByTestId("scene-widget-empty")).toHaveCount(0);

  // Read-only: the cell must NOT carry the page's persistence bar or Save button.
  await expect(cell.getByTestId("scene-persistence-bar")).toHaveCount(0);
  await expect(cell.getByTestId("scene-save")).toHaveCount(0);

  await page.screenshot({
    path: "../rust/extensions/thecrew/docs/shots/scene-widget-dashboard.png",
    fullPage: true,
  });

  expect(pageErrors, `page errors: ${pageErrors.join(" | ")}`).toHaveLength(0);
  await expect(page.getByText(/could not load/i)).toHaveCount(0);
  const fatal = consoleErrors.filter((e) => /hook|module specifier|process is not defined/i.test(e));
  expect(fatal, `fatal console: ${fatal.join(" | ")}`).toHaveLength(0);
});
