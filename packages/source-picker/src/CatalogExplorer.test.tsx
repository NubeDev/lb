// `CatalogExplorer` component tests — render every per-section state and assert the click-to-insert
// contract (system-catalog scope testing plan). Stays in the package's own unit suite (no transport
// — the explorer is pure projection over `CatalogSections`); the real deny/isolation stays proven
// by the host gateway suites (rules `AuthoringPanel.gateway`). Uses the package's existing test
// style (no jest-dom matchers — `getBy*` throws on miss, so `.toBeTruthy()`/`.toBeTruthy()` is enough).

import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { CatalogExplorer } from "./CatalogExplorer";
import type { CatalogSections } from "./loadCatalog";
import type { CatalogEntry } from "./catalog";

describe("CatalogExplorer", () => {
  it("renders a loading skeleton for an in-flight section", () => {
    const sections: CatalogSections = { series: { status: "loading" } };
    render(<CatalogExplorer sections={sections} onSelect={() => {}} />);
    expect(screen.getByLabelText("loading")).toBeTruthy();
    // The section header renders regardless of state.
    expect(screen.getByLabelText("section Series")).toBeTruthy();
  });

  it("renders 'Not permitted.' for a denied section, never a fabricated roster", () => {
    const sections: CatalogSections = {
      series: { status: "denied", error: "capability denied" },
    };
    render(<CatalogExplorer sections={sections} onSelect={() => {}} />);
    expect(screen.getByLabelText("denied")).toBeTruthy();
    expect(screen.queryByLabelText(/insert series/)).toBeNull();
  });

  it("renders a teaching empty for a ready-but-empty section", () => {
    const sections: CatalogSections = {
      series: { status: "ready", data: [] },
    };
    render(<CatalogExplorer sections={sections} onSelect={() => {}} />);
    expect(screen.getByText("No series in this workspace.")).toBeTruthy();
  });

  it("renders ready rows and fires onSelect with the picked entry", () => {
    const sections: CatalogSections = {
      series: { status: "ready", data: ["cooler.temp"] },
    };
    const onSelect = vi.fn();
    render(<CatalogExplorer sections={sections} onSelect={onSelect} />);
    fireEvent.click(screen.getByLabelText("insert series cooler.temp"));
    const expected: CatalogEntry = { kind: "series", id: "series:cooler.temp", name: "cooler.temp" };
    expect(onSelect).toHaveBeenCalledWith(expected);
  });

  it("renders the schema table→column tree; expanding a table lists its columns; clicking a column fires onSelect", () => {
    const sections: CatalogSections = {
      schema: {
        status: "ready",
        data: {
          tables: [
            { name: "flow_run", columns: [{ name: "status", type: "string" }, { name: "id", type: "string" }] },
          ],
        },
      },
    };
    const onSelect = vi.fn();
    render(<CatalogExplorer sections={sections} onSelect={onSelect} />);
    // The table header is visible immediately.
    expect(screen.getByLabelText("insert table flow_run")).toBeTruthy();
    // Columns are hidden until the table is expanded.
    expect(screen.queryByLabelText("insert column flow_run.status")).toBeNull();
    fireEvent.click(screen.getByLabelText("toggle table flow_run"));
    // Now both columns are reachable.
    fireEvent.click(screen.getByLabelText("insert column flow_run.status"));
    expect(onSelect).toHaveBeenCalledWith({
      kind: "column",
      id: "column:flow_run.status",
      table: "flow_run",
      column: "status",
    });
    // Clicking the table header yields a `table` entry (no column).
    fireEvent.click(screen.getByLabelText("insert table flow_run"));
    expect(onSelect).toHaveBeenCalledWith({
      kind: "table",
      id: "table:flow_run",
      table: "flow_run",
    });
  });

  it("clicking a datasource row fires onSelect with the name + kind + endpoint", () => {
    const sections: CatalogSections = {
      datasources: {
        status: "ready",
        data: [{ name: "timescale", kind: "postgres", endpoint: "tsdb.acme:5432" }],
      },
    };
    const onSelect = vi.fn();
    render(<CatalogExplorer sections={sections} onSelect={onSelect} />);
    fireEvent.click(screen.getByLabelText("insert datasource timescale"));
    expect(onSelect).toHaveBeenCalledWith({
      kind: "datasource",
      id: "datasource:timescale",
      name: "timescale",
      rowKind: "postgres",
      endpoint: "tsdb.acme:5432",
    });
  });

  it("skips sections the host didn't wire (absent loader ⇒ absent section)", () => {
    const sections: CatalogSections = {
      series: { status: "ready", data: ["a.b"] },
    };
    render(<CatalogExplorer sections={sections} onSelect={() => {}} />);
    // Only the Series section renders; Datasources (no loader wired) is absent.
    expect(screen.getByLabelText("section Series")).toBeTruthy();
    expect(screen.queryByLabelText("section Datasources")).toBeNull();
  });

  it("renders the new sections (channels / insights / inbox) and fires onSelect", () => {
    const sections: CatalogSections = {
      channels: { status: "ready", data: [{ id: "general" }] },
      insights: {
        status: "ready",
        data: [{ id: "i1", title: "AHU 2 anomaly", severity: "warning", status: "open" }],
      },
      inbox: { status: "ready", data: [{ id: "x1", channel: "general" }] },
    };
    const onSelect = vi.fn();
    render(<CatalogExplorer sections={sections} onSelect={onSelect} />);
    fireEvent.click(screen.getByLabelText("insert channel general"));
    expect(onSelect).toHaveBeenCalledWith({ kind: "channel", id: "channel:general", name: "general" });
    fireEvent.click(screen.getByLabelText("insert insight AHU 2 anomaly"));
    expect(onSelect).toHaveBeenCalledWith({
      kind: "insight",
      id: "insight:i1",
      title: "AHU 2 anomaly",
      severity: "warning",
      status: "open",
    });
    fireEvent.click(screen.getByLabelText("insert inbox item x1"));
    expect(onSelect).toHaveBeenCalledWith({ kind: "inbox", id: "inbox:x1", channel: "general" });
  });

  it("a denied channel.list section renders 'Not permitted.' (mandatory deny category)", () => {
    const sections: CatalogSections = {
      channels: { status: "denied", error: "denied" },
    };
    render(<CatalogExplorer sections={sections} onSelect={() => {}} />);
    expect(screen.getByLabelText("denied")).toBeTruthy();
    expect(screen.queryByLabelText(/insert channel/)).toBeNull();
  });
});
