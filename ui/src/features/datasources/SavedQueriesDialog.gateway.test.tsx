// The SavedQueriesDialog, driven against a REAL in-process gateway (datasources-ux + query scope;
// CLAUDE §9 / testing §0 — no fake backend, no `*.fake.ts`). Each test signs into a UNIQUE workspace,
// registers a real `datasource:<name>` row, persists a real `query:{ws}:{id}` record through the
// shipped `query.save` MCP verb, and drives the dialog's expand/copy row actions over the real
// `POST /mcp/call` bridge. The dialog's `onFetchText` rides the real `query.get` (lazy per row).
//
// Coverage:
//   - HEADLINE: expand a row → `query.get` fires → the read-only `SqlEditor` renders the saved SQL.
//   - COPY: copy a row → `query.get` fires → the SQL lands verbatim on the clipboard.
//   - CACHE: a second interaction on the SAME row does NOT re-fire `query.get` (one fetch per row,
//     cached in dialog state for the session).
//
// jsdom has no layout engine; CodeMirror reaches for `Range.getClientRects`, so the same polyfill the
// `AuthoringPanel.gateway.test.tsx` uses is stubbed in `beforeAll`.

import { describe, expect, it, beforeAll, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { SavedQueriesDialog } from "./SavedQueriesDialog";
import { datasourceTarget, useDatasourceQueries } from "./useDatasourceQueries";
import { useRealGateway, signInReal } from "@/test/gateway-session";
import { addDatasource } from "@/lib/datasources";
import { saveQuery } from "@/lib/queries";
import * as ipc from "@/lib/ipc/invoke";

let n = 0;
const nextWs = () => `saved-q-${n++}`;

beforeAll(() => {
  useRealGateway();
  // jsdom has no layout engine; polyfill the Range measurement methods CodeMirror reaches for
  // (mirror of AuthoringPanel.gateway.test.tsx — without this, the read-only `SqlEditor` won't mount
  // its `.cm-content` and the textContent assertion below can't see the rendered SQL).
  if (!Range.prototype.getClientRects) {
    Range.prototype.getClientRects = () =>
      ({ length: 0, item: () => null, [Symbol.iterator]: function* () {} }) as unknown as DOMRectList;
  }
  if (!Range.prototype.getBoundingClientRect) {
    Range.prototype.getBoundingClientRect = () =>
      ({ x: 0, y: 0, width: 0, height: 0 }) as DOMRect;
  }
});

/** Register a real federation `datasource:<name>` row in the session workspace (the roster path; no
 *  sidecar is spawned in this env — the dialog never runs a query, so federation never starts). */
async function registerSource(source: string): Promise<void> {
  await addDatasource({
    name: source,
    kind: "sqlite",
    endpoint: "127.0.0.1:0",
    dsn: "/tmp/lb-saved-queries-demo.db",
  });
}

/** Persist a real `query:{ws}:{id}` row against `datasource:<source>` through the shipped
 *  `query.save` MCP verb. The dialog lists + lazy-loads these via `query.get` over the same bridge. */
async function seedSavedQuery(args: {
  source: string;
  id: string;
  name?: string;
  sql: string;
}): Promise<void> {
  await saveQuery({
    id: args.id,
    name: args.name,
    lang: "raw",
    text: args.sql,
    target: datasourceTarget(args.source),
  });
}

/** An `ipc.invoke` counter that DELEGATES to the real transport (observe, never fake — rule 9).
 *  Mirrors the QueryWorkbench.gateway.test.tsx helper so we can assert `query.get` call counts. */
function viaCounter() {
  const real = ipc.invoke;
  const byTool = new Map<string, number>();
  const spy = vi.spyOn(ipc, "invoke").mockImplementation(((cmd: string, args?: Record<string, unknown>) => {
    if (cmd === "mcp_call") {
      const tool = (args?.tool as string) ?? "?";
      byTool.set(tool, (byTool.get(tool) ?? 0) + 1);
    }
    return real(cmd, args);
  }) as typeof ipc.invoke);
  void spy;
  return {
    countTool: (tool: string) => byTool.get(tool) ?? 0,
    restore: () => spy.mockRestore(),
  };
}

// `vi` is imported at the top so the spy is in scope for `viaCounter`.

/** Mount the dialog against the real per-source hook (the same composition QueryWorkbench uses). The
 *  hook owns the `query.list` filter to `target === datasource:<source>`; the dialog rides its
 *  `load(id)` for the lazy text fetch. */
function Harness({ source }: { source: string }) {
  const saved = useDatasourceQueries(source);
  return (
    <SavedQueriesDialog
      queries={saved.queries}
      loading={saved.loading}
      error={saved.error}
      onLoad={() => {
        /* the workbench's load path is exercised in QueryWorkbench.gateway.test */
      }}
      onDelete={(id) => saved.remove(id)}
      onFetchText={(id) => saved.load(id).then((q) => q.text)}
    />
  );
}

describe("SavedQueriesDialog (real gateway)", () => {
  it("HEADLINE: expand a row → query.get fires → read-only SqlEditor renders the saved SQL", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    const source = "demo-buildings";
    await registerSource(source);

    const SQL = "SELECT id, name FROM buildings LIMIT 10";
    await seedSavedQuery({ source, id: "top-buildings", name: "Top buildings", sql: SQL });

    const counted = viaCounter();
    render(<Harness source={source} />);

    await user.click(await screen.findByRole("button", { name: "open saved query" }));
    const row = await screen.findByRole("button", { name: "expand Top buildings" });
    await user.click(row);

    // `query.get` fires once to lazy-load the row's text.
    await waitFor(() => expect(counted.countTool("query.get")).toBeGreaterThanOrEqual(1));

    // The read-only SqlEditor mounts and CodeMirror renders the saved SQL into `.cm-content`.
    await waitFor(() => {
      const editor = document.querySelector('[aria-label="saved query text top-buildings"]');
      const cm = editor?.querySelector(".cm-content")?.textContent ?? "";
      expect(cm).toContain("SELECT");
      expect(cm).toContain("buildings");
    });
    counted.restore();
  }, 30_000);

  it("COPY: copy a row → query.get fires → SQL lands verbatim on the clipboard", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    const source = "demo-units";
    await registerSource(source);

    const SQL = "SELECT count(*) AS n FROM units";
    await seedSavedQuery({ source, id: "unit-count", name: "Unit count", sql: SQL });

    const writes: string[] = [];
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: (text: string) => {
          writes.push(text);
          return Promise.resolve();
        },
      },
    });

    const counted = viaCounter();
    render(<Harness source={source} />);

    await user.click(await screen.findByRole("button", { name: "open saved query" }));
    await user.click(await screen.findByRole("button", { name: "copy Unit count" }));

    // The exact SQL is on the clipboard (lazy `query.get` → text → writeText).
    await waitFor(() => expect(writes[0]).toBe(SQL));
    expect(counted.countTool("query.get")).toBeGreaterThanOrEqual(1);
    counted.restore();
  }, 30_000);

  it("CACHE: a second interaction on the SAME row does NOT re-fire query.get", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    const source = "demo-cache";
    await registerSource(source);

    await seedSavedQuery({ source, id: "cached", name: "Cached", sql: "SELECT 1" });

    const writes: string[] = [];
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: (text: string) => {
          writes.push(text);
          return Promise.resolve();
        },
      },
    });

    const counted = viaCounter();
    render(<Harness source={source} />);

    await user.click(await screen.findByRole("button", { name: "open saved query" }));
    // Expand first — fires query.get once.
    await user.click(await screen.findByRole("button", { name: "expand Cached" }));
    await waitFor(() => expect(counted.countTool("query.get")).toBe(1));

    // Copy next — uses the cached text, NOT a second query.get.
    await user.click(screen.getByRole("button", { name: "copy Cached" }));
    await waitFor(() => expect(writes[0]).toBe("SELECT 1"));
    expect(counted.countTool("query.get")).toBe(1);
    counted.restore();
  }, 30_000);
});
