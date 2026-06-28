// E2E: an installed extension's `[[widget]]` tile is added from the dashboard palette and mounts
// IN-PROCESS in the BUILT shell (widget-palette scope; debugging/frontend/ext-widget-iframe-tier-
// cannot-resolve-bare-react.md).
//
// This is the regression guard for the trust-tier fix. `proof-panel`'s remote externalises React to be
// resolved by the shell import map — so it can ONLY render in-process; the old "non-allow-listed →
// iframe" routing left it blank (`Failed to resolve module specifier "react"`). That failure only
// surfaces in a REAL browser loading the gateway-served remote, so a jsdom unit test can't catch it.
// Here a real Chromium loads the production shell on :4173, logs in against the REAL node on :8080,
// opens the dashboard builder, picks "proof-panel · Proof Ping" from the new "Extension widgets"
// palette group, adds it, and asserts the tile mounted in-process with the host's single React and
// rendered the REAL `proof.demo` value — no error wrapper, no hook-call crash.

import { test, expect, type ConsoleMessage } from "@playwright/test";

const SHELL = process.env.LB_SHELL_URL ?? "http://127.0.0.1:4173";
const GATEWAY = process.env.VITE_GATEWAY_URL ?? "http://127.0.0.1:8080";

test.beforeAll(async ({ request }) => {
  // Fail fast with a clear message if the out-of-band servers (built shell + real node) aren't up,
  // and seed the series the tile reads so its value is non-empty.
  const shell = await request.get(SHELL).catch(() => null);
  if (!shell || !shell.ok()) {
    throw new Error(`built shell not reachable at ${SHELL} — run 'make ui-preview' first`);
  }
  const remote = await request
    .get(`${GATEWAY}/extensions/proof-panel/ui/remoteEntry.js`)
    .catch(() => null);
  if (!remote || !remote.ok()) {
    throw new Error(
      `proof-panel remote not served at ${GATEWAY} — run 'make publish-ext EXT=proof-panel' (node must be up)`,
    );
  }
  // Seed proof.demo through the real ingest path so the tile shows a real committed value.
  const login = await request.post(`${GATEWAY}/login`, {
    data: { user: "user:ada", workspace: "acme" },
  });
  const token = (await login.json()).token as string;
  await request.post(`${GATEWAY}/ingest`, {
    headers: { authorization: `Bearer ${token}` },
    data: {
      samples: [
        { series: "proof.demo", producer: "e2e", ts: 1, seq: 1, payload: 21, labels: null, qos: "best-effort" },
      ],
    },
  });
});

test("a Proof Ping tile added from the palette mounts in-process with the real proof.demo value", async ({
  page,
}) => {
  const consoleErrors: string[] = [];
  const pageErrors: string[] = [];
  page.on("console", (msg: ConsoleMessage) => {
    if (msg.type() === "error") consoleErrors.push(msg.text());
  });
  page.on("pageerror", (err) => pageErrors.push(err.message));

  // 1) Load the built shell → log in as user:ada / acme against the real node.
  await page.goto(SHELL, { waitUntil: "networkidle" });
  await page.getByLabel("identity").fill("user:ada");
  await page.getByLabel("workspace").fill("acme");
  await page.getByLabel("sign in").click();

  // 2) Open the Dashboards surface and create a fresh dashboard (auto-selects on create).
  await page.getByRole("button", { name: "Dashboards", exact: true }).click();
  const titleInput = page.getByLabel("new dashboard title");
  await expect(titleInput).toBeVisible({ timeout: 15_000 });
  await titleInput.fill("Widget E2E");
  await page.getByLabel("create dashboard").click();

  // 3) The builder is shown (the session holds mcp:dashboard.save:call → the add affordance renders).
  //    Pick the packaged tile from the new "Extension widgets" palette group.
  const source = page.getByLabel("widget source");
  await expect(source).toBeVisible({ timeout: 15_000 });
  // Exact label — the picker also lists "proof-panel · Proof Ping Live" (the 2nd [[widget]]), so a
  // substring match would be ambiguous.
  await expect(
    source.locator("option").filter({ hasText: /^proof-panel · Proof Ping$/ }),
  ).toHaveCount(1, { timeout: 15_000 });
  await source.selectOption({ label: "proof-panel · Proof Ping" });

  // 4) The view chooser is hidden (a packaged tile is its own view), and the PREVIEW mounts the real
  //    ExtWidget IN-PROCESS — the host element carries data-tier="in-process", never an iframe.
  await expect(page.getByLabel("widget view")).toHaveCount(0);
  const previewHost = page.locator('[data-ext-widget="proof-panel"][data-tier="in-process"]').first();
  await expect(previewHost).toBeAttached({ timeout: 15_000 });
  // The federated remote actually rendered its tile (proves React resolved against the shell singleton)
  // and read a REAL `proof.demo` value over the bridge — a number, never the "no value"/"no access"
  // state. (The exact value is whatever `series.latest` holds for this workspace, not a fixed literal.)
  await expect(previewHost.locator("[data-proof-widget]")).toBeVisible({ timeout: 15_000 });
  await expect(previewHost.getByLabel("proof widget value")).toHaveText(/^\d+(\.\d+)?$/, {
    timeout: 15_000,
  });

  // 5) Add it to the dashboard → it persists and re-renders the same in-process tile in a grid cell.
  await page.getByLabel("add widget").click();
  const cellHost = page.locator('[data-ext-widget="proof-panel"][data-tier="in-process"]');
  await expect(cellHost.first()).toBeAttached({ timeout: 15_000 });
  await expect(cellHost.first().getByLabel("proof widget value")).toHaveText(/^\d+(\.\d+)?$/, {
    timeout: 15_000,
  });

  // 6) No iframe was used for the extension widget, and the shell's error wrapper is absent.
  await expect(page.locator("[data-widget-iframe]")).toHaveCount(0);
  await expect(page.getByText(/could not load/i)).toHaveCount(0);

  // 7) Screenshot the mounted tile for the session doc.
  await page.screenshot({ path: "e2e/__screenshots__/dashboard-widget-mounted.png", fullPage: true });

  // 8) No "Invalid hook call" / "more than one copy of React" / "Failed to resolve module specifier"
  //    (the exact failure this fix removes) and no uncaught errors.
  const allErrors = [...consoleErrors, ...pageErrors];
  const fatal = allErrors.filter(
    (e) =>
      /Invalid hook call/i.test(e) ||
      /more than one copy of React/i.test(e) ||
      /Failed to resolve module specifier/i.test(e),
  );
  expect(fatal, `fatal federation errors:\n${fatal.join("\n")}`).toHaveLength(0);
});
