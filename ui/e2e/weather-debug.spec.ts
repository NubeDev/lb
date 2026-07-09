import { test, expect } from "@playwright/test";

const GATEWAY = "http://127.0.0.1:8080";
const APP = "http://localhost:5173";

// Throwaway: drive the LIVE Energy dashboard exactly as the browser does, with a FRESH token,
// and capture every /mcp/call the panel makes + the server's reply. Find why "no access to this source".
test("energy dashboard weather panel — live wire", async ({ page, request }) => {
  const login = await request.post(`${GATEWAY}/login`, {
    data: { user: "user:ada", workspace: "acme" },
  });
  const body = await login.json();
  const token: string = body.token;
  console.log("LOGIN caps has weather:", JSON.stringify(body).includes("weather.current"));

  // capture wire
  page.on("request", (r) => {
    if (r.url().includes("/mcp/call")) console.log(">> POST", r.url(), r.postData());
  });
  page.on("response", async (r) => {
    if (r.url().includes("/mcp/call")) {
      let t = "";
      try { t = await r.text(); } catch {}
      console.log("<< ", r.status(), t.slice(0, 400));
    }
  });

  // Log in the REAL way through the form (fresh token, complete session), then navigate.
  await page.goto(APP);
  await page.evaluate(() => localStorage.clear());
  await page.goto(APP);
  await page.getByLabel("sign in").click();
  await page.waitForTimeout(1500);

  await page.goto(`${APP}/#/t/acme/dashboards?d=energy&from=2026-06-09&to=2026-07-09`);
  await page.waitForTimeout(4000);

  // dump what the session actually stored (does it carry caps?)
  const sess = await page.evaluate(() => localStorage.getItem("lb.session"));
  console.log("STORED SESSION:", sess?.slice(0, 300));

  const bodyText = await page.evaluate(() => document.body.innerText);
  console.log("PAGE TEXT:\n", bodyText.slice(0, 1500));
  await page.screenshot({ path: "e2e/__screenshots__/weather-debug.png", fullPage: false });
});
