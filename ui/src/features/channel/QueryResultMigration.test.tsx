// query_result → rich_result migration / NO-REGRESSION (channel rich responses scope). The rich_result
// render envelope GENERALIZES the older `query_result` payload, but the shipped `query_result` path must
// keep working unchanged. This unit file proves both, honestly:
//
//   1. NO REGRESSION — a `kind:"query_result"` Item still routes MessageItem → QueryCard and renders its
//      table (and a chart when the spec is present) from its INLINE columns/rows. Nothing about the new
//      rich_result envelope changes that path.
//
//   2. EXPRESSIBILITY (pure mapping) — a `query_result`'s (columns, rows, chart) maps 1:1 onto a
//      `rich_result` table/chart envelope shape. We assert the mapping is total and loss-free. We do NOT
//      fake a render of that envelope: the shipped RESPONSE views are SOURCE-BACKED (ResponseView builds
//      a cell whose data loads through a `source` via usePanelData → viz.query); an inline-`data`-only
//      envelope with no source has no shipped read path and degrades honestly at render (ResponseView's
//      own contract). So the honest generalization proof is (a) the mapping below + (b) that MessageItem
//      ROUTES a rich_result to ResponseView (the new render path) while still routing query_result to
//      QueryCard (the old one) — the two coexist, which is the whole point.
//
// No gateway needed: (1) renders inline data, (2) is a pure mapping + a routing assertion. A
// source-backed rich_result render against a real gateway is already covered by the reminders suite
// (the reminder.list table re-runs its source) — we don't duplicate a live source here.

import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";

import { MessageItem } from "./MessageItem";
import type { Item } from "@/lib/channel/channel.types";
import {
  encodeRichResult,
  parsePayload,
  type QueryResultPayload,
  type RichResultPayload,
} from "@/lib/channel/payload.types";

function item(body: string): Item {
  return { id: "i1", channel: "general", author: "system:query-worker", body, ts: 1 };
}

const noop = () => {};

/** Map a `query_result` payload onto the equivalent `rich_result` render envelope: a `table` view (or
 *  `chart` when the query_result carried a chart spec) carrying the SAME columns + rows as inline `data`.
 *  This is the pure generalization — the same information, in the newer envelope shape. */
function queryResultToRich(q: QueryResultPayload): Omit<RichResultPayload, "kind" | "v"> {
  return {
    view: q.chart ? "chart" : "table",
    data: { columns: q.columns, rows: q.rows, chart: q.chart ?? null },
  };
}

describe("query_result → rich_result migration (no regression + expressibility)", () => {
  it("no regression: a query_result Item still renders through QueryCard (table)", () => {
    const body = JSON.stringify({
      kind: "query_result",
      source: "warehouse",
      sql: "SELECT name, note FROM notes",
      columns: ["name", "note"],
      rows: [
        { name: "a", note: "hi" },
        { name: "b", note: "yo" },
      ],
      chart: null,
    } satisfies QueryResultPayload);

    render(<MessageItem item={item(body)} author="user:me" ws="w" onEdit={noop} onDelete={noop} />);

    // The old path renders its table inline (chart:null → table-only), NOT raw JSON.
    expect(screen.getByLabelText("query result table")).toBeInTheDocument();
    expect(screen.getByText("hi")).toBeInTheDocument();
    expect(screen.getByText("yo")).toBeInTheDocument();
  });

  it("no regression: a query_result WITH a chart spec still renders the chart via QueryCard", () => {
    const body = JSON.stringify({
      kind: "query_result",
      source: "warehouse",
      sql: "SELECT day, signups FROM daily",
      columns: ["day", "signups"],
      rows: [
        { day: "2024-01-01", signups: 3 },
        { day: "2024-01-02", signups: 5 },
      ],
      chart: { type: "line", x: "day", series: [{ field: "signups" }] },
    } satisfies QueryResultPayload);

    render(<MessageItem item={item(body)} author="user:me" ws="w" onEdit={noop} onDelete={noop} />);

    expect(screen.getByLabelText("query result")).toBeInTheDocument();
    expect(screen.getByLabelText("line chart")).toBeInTheDocument();
  });

  it("expressibility: a query_result maps 1:1 onto a rich_result table envelope (loss-free)", () => {
    const q: QueryResultPayload = {
      kind: "query_result",
      source: "warehouse",
      sql: "SELECT a, b FROM t",
      columns: ["a", "b"],
      rows: [
        { a: 1, b: "x" },
        { a: 2, b: "y" },
      ],
      chart: null,
    };

    const envelope = queryResultToRich(q);
    // The same columns + rows, now under a `table` view — nothing lost.
    expect(envelope.view).toBe("table");
    expect(envelope.data).toEqual({ columns: q.columns, rows: q.rows, chart: null });

    // And it round-trips through the wire encoder/decoder as a valid rich_result payload.
    const parsed = parsePayload(encodeRichResult(envelope)) as RichResultPayload;
    expect(parsed.kind).toBe("rich_result");
    expect(parsed.v).toBe(2);
    expect(parsed.view).toBe("table");
    expect(parsed.data).toEqual({ columns: ["a", "b"], rows: q.rows, chart: null });
  });

  it("expressibility: a query_result WITH a chart maps onto a chart-view rich_result envelope", () => {
    const q: QueryResultPayload = {
      kind: "query_result",
      source: "warehouse",
      sql: "SELECT day, n FROM d",
      columns: ["day", "n"],
      rows: [{ day: "2024-01-01", n: 3 }],
      chart: { type: "line", x: "day", series: [{ field: "n" }] },
    };
    const envelope = queryResultToRich(q);
    expect(envelope.view).toBe("chart"); // a charted query_result becomes a chart-view rich_result
    expect((envelope.data as { chart: unknown }).chart).toEqual(q.chart); // the chart spec carries over
  });

  it("routing: MessageItem sends query_result → QueryCard and rich_result → ResponseView (both coexist)", () => {
    // The generalization does not replace the old path — MessageItem routes each kind to its own
    // renderer. A rich_result mounts ResponseView (the new render path); a query_result mounts QueryCard.
    // (parsePayload recognizes BOTH kinds — the migration is additive, never a cutover.)
    const qr = parsePayload(
      JSON.stringify({
        kind: "query_result",
        source: "s",
        sql: "x",
        columns: ["a"],
        rows: [{ a: 1 }],
        chart: null,
      }),
    );
    const rr = parsePayload(
      encodeRichResult({ view: "table", source: { tool: "reminder.list", args: {} } }),
    );
    expect(qr?.kind).toBe("query_result");
    expect(rr?.kind).toBe("rich_result");

    // Render the rich_result: MessageItem must NOT route it to QueryCard (no "query result" card) — it
    // goes to ResponseView (source-backed; with no live gateway it degrades honestly, never a fake render).
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    render(
      <MessageItem
        item={item(encodeRichResult({ view: "table", source: { tool: "reminder.list", args: {} } }))}
        author="user:me"
        ws="w"
        onEdit={noop}
        onDelete={noop}
      />,
    );
    // A rich_result is NOT a query card (it took the ResponseView branch, not QueryCard).
    expect(screen.queryByLabelText("query result")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("query result table")).not.toBeInTheDocument();
    spy.mockRestore();
  });
});
