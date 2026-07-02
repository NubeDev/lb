// THROWAWAY probe: navigate to the Control Engine page and report why edges don't render.
import { test, type ConsoleMessage } from "@playwright/test";

const SHELL = process.env.LB_SHELL_URL ?? "http://127.0.0.1:5173";

test("probe CE edges", async ({ page }) => {
  const logs: string[] = [];
  page.on("console", (m: ConsoleMessage) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on("pageerror", (e) => logs.push(`[pageerror] ${e.message}`));

  // Capture the tree/mcp responses so we see exactly what the page received.
  const mcp: Array<{ tool: string; edges: number; nodes: number; snippet: string }> = [];
  page.on("response", async (res) => {
    const u = res.url();
    if (!u.includes("/mcp/call")) return;
    try {
      const req = res.request();
      const post = req.postDataJSON?.() as { tool?: string } | undefined;
      if (post?.tool !== "control-engine.tree") return;
      const body = await res.json();
      const r = body.result ?? body;
      mcp.push({
        tool: post.tool,
        nodes: Array.isArray(r.nodes) ? r.nodes.length : -1,
        edges: Array.isArray(r.edges) ? r.edges.length : -1,
        snippet: JSON.stringify(r.edges?.[0] ?? null).slice(0, 300),
      });
    } catch { /* ignore */ }
  });

  await page.goto(SHELL, { waitUntil: "networkidle" });
  await page.getByLabel("identity").fill("user:ada");
  await page.getByLabel("workspace").fill("acme");
  await page.getByLabel("sign in").click();
  await page.waitForTimeout(1500);

  // Navigate via the shell's own nav slot.
  await page.getByRole("button", { name: /Control Engine/i }).first().click().catch(async () => {
    await page.getByText(/Control Engine/i).first().click();
  });
  await page.waitForTimeout(7000);

  const report = await page.evaluate(() => {
    const q = (s: string) => Array.from(document.querySelectorAll(s));
    const nodes = q(".react-flow__node");
    const edges = q(".react-flow__edge");
    const handles = q(".react-flow__handle");
    return {
      url: location.href,
      nodeCount: nodes.length,
      nodeIds: nodes.map((n) => n.getAttribute("data-id")),
      edgeCount: edges.length,
      edgeIds: edges.map((e) => e.getAttribute("data-id")),
      handleCount: handles.length,
      handleSample: handles.slice(0, 12).map((h) => ({
        id: h.getAttribute("data-handleid"),
        type: h.classList.contains("source") ? "source" : h.classList.contains("target") ? "target" : "?",
      })),
      hasCanvas: !!document.querySelector(".react-flow"),
      // The two specific handles the missing edge needs.
      srcHandle1000115: q('.react-flow__handle[data-handleid="1000115"]').map((h) => ({
        type: h.classList.contains("source") ? "source" : h.classList.contains("target") ? "target" : "?",
        nodeId: h.closest(".react-flow__node")?.getAttribute("data-id"),
      })),
      dstHandle1000122: q('.react-flow__handle[data-handleid="1000122"]').map((h) => ({
        type: h.classList.contains("source") ? "source" : h.classList.contains("target") ? "target" : "?",
        nodeId: h.closest(".react-flow__node")?.getAttribute("data-id"),
      })),
      allHandleIds: q(".react-flow__handle").map((h) => h.getAttribute("data-handleid")),
      pageText: (document.querySelector("[data-control-engine-page]")?.textContent ?? "").slice(0, 300),
    };
  });

  console.log("=== CE PROBE REPORT ===");
  console.log(JSON.stringify(report, null, 2));
  console.log("=== MCP control-engine.tree responses ===");
  console.log(JSON.stringify(mcp, null, 2));
  console.log("=== CONSOLE (errors/warnings) ===");
  console.log(logs.filter((l) => /error|warn|fail/i.test(l)).slice(-40).join("\n"));
});
