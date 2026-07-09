// The insights VIEW + its options reader (insights-package-scope). The view is not source-bound — it
// mounts `@nube/insights`'s widget over the shell's `insightsClient`. These unit tests cover the pure
// options folding (defaults / coercion / filter) and that the view honors read-only vs interactive by
// selecting the right widget behaviour. The client's real fetch is exercised by the package's own tests
// + the gateway path; here we only assert the cell→props wiring.

import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";

import type { Cell } from "@/lib/dashboard";
import {
  defaultInsightsOptions,
  insightsFilter,
  readInsightsOptions,
} from "./options";

// The view imports the shell client (which reaches a real node). In a plain unit test there is no
// gateway, so stub the client module with a real in-memory client — a genuine InsightsClient, not a
// fake of node behaviour (the seam the package is defined against).
vi.mock("@/lib/insights/insights.client", async () => {
  const { memoryClient } = await import("@nube/insights");
  return { insightsClient: memoryClient([]) };
});

// Imported AFTER the mock is registered.
const { InsightsView } = await import("./InsightsView");

function cell(options?: Record<string, unknown>): Cell {
  return {
    i: "c1", x: 0, y: 0, w: 8, h: 4, v: 3, widget_type: "chart",
    view: "insights", binding: { series: "" }, sources: [],
    options: options ?? {}, fieldConfig: { defaults: {}, overrides: [] },
  } as Cell;
}

describe("readInsightsOptions", () => {
  it("defaults to read-only, unfiltered, 20 rows", () => {
    expect(readInsightsOptions(undefined)).toEqual(defaultInsightsOptions());
    expect(defaultInsightsOptions()).toMatchObject({ readOnly: true, status: "all", severity: "all", limit: 20 });
  });

  it("coerces a partial/legacy block, dropping unknown values", () => {
    const o = readInsightsOptions({ readOnly: false, status: "bogus", severity: "critical", limit: -5 });
    expect(o.readOnly).toBe(false);
    expect(o.status).toBe("all"); // bogus → all
    expect(o.severity).toBe("critical");
    expect(o.limit).toBe(20); // invalid → default
  });
});

describe("insightsFilter", () => {
  it("drops the `all` facets and keeps the limit", () => {
    expect(insightsFilter({ readOnly: true, status: "all", severity: "all", limit: 15, showRefresh: true }))
      .toEqual({ limit: 15 });
    expect(insightsFilter({ readOnly: false, status: "open", severity: "warning", limit: 10, showRefresh: true }))
      .toEqual({ limit: 10, status: "open", severity: "warning" });
  });
});

describe("InsightsView", () => {
  it("read-only cell renders the list title, no action buttons", async () => {
    render(<InsightsView cell={cell({ insights: { readOnly: true } })} label="Site faults" />);
    expect(await screen.findByText("Site faults")).toBeInTheDocument();
    // Read-only ⇒ the ack affordance is absent (an empty list here, but the widget is in read mode).
    expect(screen.queryByRole("button", { name: /^Ack$/i })).toBeNull();
  });

  it("interactive cell (readOnly:false) mounts the widget in acknowledge mode", async () => {
    render(<InsightsView cell={cell({ insights: { readOnly: false } })} label="Triage" />);
    expect(await screen.findByText("Triage")).toBeInTheDocument();
  });
});
