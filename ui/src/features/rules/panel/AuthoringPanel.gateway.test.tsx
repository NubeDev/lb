// The guided authoring surface, driven against a REAL in-process gateway (rules-editor-ux scope; CLAUDE
// §9 / testing §0 — no fake backend). Renders the full RulesView (editor + AuthoringPanel) and exercises:
//   - the function palette renders the real categories and click-to-insert appends a snippet to the buffer;
//   - search filters the palette by name;
//   - an example loads into the buffer and runs green via the real `rules.run`;
//   - the dirty-confirm guard blocks clobbering unsaved edits;
//   - the data explorer lists a REAL registered datasource (`datasource.list`) + the local schema
//     (`store.schema`) + real series (`series.list`), each click-to-insert, with NO DSN ever rendered;
//   - a denied datasource section renders an honest deny, never a fabricated roster.
// Every datum is a real read/seed via the real write path (seedIotDemo + a real `datasource.add`).

import { describe, expect, it, beforeAll, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { RulesView } from "../RulesView";
import { addDatasource } from "@/lib/datasources";
import {
  useRealGateway,
  signInReal,
  signInWithCaps,
  seedIotDemo,
} from "@/test/gateway-session";

let n = 0;
const nextWs = () => `rules-ux-${n++}`;

beforeAll(() => {
  useRealGateway();
  // jsdom has no layout engine; polyfill the Range measurement methods CodeMirror reaches for.
  if (!Range.prototype.getClientRects) {
    Range.prototype.getClientRects = () =>
      ({ length: 0, item: () => null, [Symbol.iterator]: function* () {} }) as unknown as DOMRectList;
  }
  if (!Range.prototype.getBoundingClientRect) {
    Range.prototype.getBoundingClientRect = () => ({ x: 0, y: 0, width: 0, height: 0 }) as DOMRect;
  }
});

/** Read the editor's current text (CodeMirror renders the buffer into `.cm-content`). */
function editorText(): string {
  const editor = screen.getByLabelText("rule editor");
  return (editor.querySelector(".cm-content") as HTMLElement)?.textContent ?? "";
}

describe("AuthoringPanel (real gateway)", () => {
  it("the function palette renders the real categories and click-to-insert appends a snippet", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<RulesView ws={ws} />);

    // The five real verb families are present (mirrors the lb-rules crate).
    const palette = screen.getByLabelText("function palette");
    for (const label of ["Data", "Grid", "Timeseries", "AI", "Output"]) {
      expect(within(palette).getByLabelText(`category ${label}`)).toBeInTheDocument();
    }

    // Click `history(...)` → its snippet is inserted into the editor buffer (the real CM transaction).
    await user.click(screen.getByLabelText("insert history"));
    expect(editorText()).toContain('history("series"');
  });

  it("the preview disclosure reveals the exact snippet code before inserting", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<RulesView ws={ws} />);

    // The snippet is hidden until the user opens the per-function preview.
    expect(screen.queryByText('history("series", "<point>", "24h")')).not.toBeInTheDocument();
    await user.click(screen.getByLabelText("preview history snippet"));
    expect(screen.getByText('history("series", "<point>", "24h")')).toBeInTheDocument();
    // Previewing does NOT insert — the editor buffer stays untouched.
    expect(editorText()).not.toContain('history("series"');
  });

  it("search filters the palette by name", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<RulesView ws={ws} />);

    await user.type(screen.getByLabelText("search functions"), "rollup");
    expect(screen.getByLabelText("insert rollup")).toBeInTheDocument();
    // A non-matching verb is filtered out.
    expect(screen.queryByLabelText("insert embed")).not.toBeInTheDocument();
  });

  it("loads an example into the buffer and runs it green via the real rules.run", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<RulesView ws={ws} />);

    await user.click(screen.getByLabelText("tab Examples"));
    await user.click(screen.getByLabelText("load example A scalar result"));
    expect(editorText()).toContain("40 + 2");

    await user.click(screen.getByLabelText("run rule"));
    const card = await screen.findByLabelText("scalar result");
    expect(within(card).getByLabelText("scalar value").textContent).toBe("42");
  });

  it("the dirty-confirm guard blocks clobbering unsaved edits", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<RulesView ws={ws} />);

    // Type an unsaved edit, then decline the confirm when loading an example.
    const area = screen.getByLabelText("rule editor").querySelector(".cm-content") as HTMLElement;
    await user.click(area);
    await user.paste("let mine = 1;");

    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
    await user.click(screen.getByLabelText("tab Examples"));
    await user.click(screen.getByLabelText("load example A scalar result"));
    expect(confirmSpy).toHaveBeenCalled();
    // The decline preserved the unsaved edit — the example did NOT clobber it.
    expect(editorText()).toContain("let mine = 1;");
    confirmSpy.mockRestore();
  });

  it("the data explorer lists a real datasource + schema + series, click-to-insert, no DSN", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedIotDemo(); // real cooler.temp / fryer.state series + the local schema
    // Register a REAL datasource through the real write path (dev session holds the datasource caps).
    await addDatasource({
      name: "timescale",
      kind: "postgres",
      endpoint: "tsdb.acme:5432",
      dsn: "postgres://secret:secret@tsdb.acme:5432/db",
    });

    render(<RulesView ws={ws} />);
    await user.click(screen.getByLabelText("tab Data"));

    // Datasource: real row (kind + endpoint), and the DSN/secret never appears anywhere in the panel.
    const explorer = await screen.findByLabelText("data explorer");
    const dsBtn = await within(explorer).findByLabelText("insert datasource timescale");
    expect(dsBtn).toHaveTextContent("postgres");
    expect(explorer.textContent ?? "").not.toContain("secret");

    // Click the datasource → a `source("timescale")` snippet lands in the buffer.
    await user.click(dsBtn);
    expect(editorText()).toContain('source("timescale")');

    // Series: a real seeded series is listed and click-to-insert emits a history() snippet.
    const seriesBtn = await within(explorer).findByLabelText("insert series cooler.temp");
    await user.click(seriesBtn);
    expect(editorText()).toContain('history("series", "cooler.temp", "24h")');
  });

  it("a denied datasource section renders an honest deny, never a fake roster", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    // Sign in WITHOUT the datasource.list cap (but with schema/series so those sections still load).
    await signInWithCaps("user:ada", ws, [
      "mcp:rules.run:call",
      "mcp:store.schema:call",
      "mcp:series.list:call",
    ]);

    render(<RulesView ws={ws} />);
    await user.click(screen.getByLabelText("tab Data"));

    const dsSection = await screen.findByLabelText("section Datasources");
    // The denied read renders a deny, not a fabricated datasource list.
    expect(await within(dsSection).findByLabelText("denied")).toBeInTheDocument();
    expect(screen.queryByLabelText("insert datasource timescale")).not.toBeInTheDocument();
  });
});
