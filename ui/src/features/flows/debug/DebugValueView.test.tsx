// Unit tests for the debug value renderer (debug-node-scope). The renderer is pure (a DebugMessage
// in → React out), so jsdom is enough — no gateway. The SSE transport is proven at the Rust layer
// (`rust/crates/host/tests/flows_debug_test.rs`); here we prove the type-aware rendering + the
// long-content auto-collapse (Decision 6) the panel chrome depends on.

import { describe, expect, it } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { DebugValueView } from "./DebugValueView";
import { DebugMessageRow } from "./DebugMessageRow";
import type { DebugMessage } from "@/lib/flows";

describe("DebugValueView — format dispatch", () => {
  it("renders a json value as the collapsible tree", () => {
    const { container } = render(
      <DebugValueView value={{ a: 1, b: [2, 3] }} format="json" />,
    );
    // The tree mounts (labelled wrapper) and shows the top-level keys + values.
    expect(container.querySelector('[aria-label="debug json tree"]')).toBeTruthy();
    // ReactJson renders keys without quotes (quotesOnKeys=false); assert the value lands.
    expect(screen.getByText("1")).toBeInTheDocument();
  });

  it("renders a text value as a <pre> with the string body", () => {
    const { container } = render(<DebugValueView value="hello world" format="text" />);
    const pre = container.querySelector("pre");
    expect(pre).toBeTruthy();
    expect(pre?.textContent).toBe("hello world");
  });

  it("renders null as the text 'null'", () => {
    const { container } = render(<DebugValueView value={null} format="text" />);
    expect(container.querySelector("pre")?.textContent).toBe("null");
  });

  it("renders markdown as rendered markdown (heading)", () => {
    const { container } = render(<DebugValueView value="# Title\n\nbody" format="markdown" />);
    // react-markdown turns the `# Title` line into an <h1>.
    const h1 = container.querySelector("h1");
    expect(h1).toBeTruthy();
    expect(h1?.textContent).toMatch(/Title/);
  });
});

describe("DebugValueView — long-content auto-collapse (Decision 6)", () => {
  const longString = "x".repeat(3000);

  it("renders in full when under the collapse threshold", () => {
    const { container } = render(
      <DebugValueView value="short" format="text" collapseBytes={2048} />,
    );
    // No "show more" disclosure appears; the value renders directly.
    expect(screen.queryByLabelText("show more")).toBeNull();
    expect(container.querySelector("pre")?.textContent).toBe("short");
  });

  it("collapses a long value and expands on 'show more'", () => {
    render(<DebugValueView value={longString} format="text" collapseBytes={2048} />);
    // Collapsed by default: the disclosure is present, the full text is hidden behind it.
    const more = screen.getByLabelText("show more");
    expect(more).toBeTruthy();
    fireEvent.click(more);
    // Expanded now — the disclosure flips and the value shows.
    expect(screen.getByLabelText("show less")).toBeTruthy();
  });

  it("never collapses when collapseBytes is 0", () => {
    render(<DebugValueView value={longString} format="text" collapseBytes={0} />);
    expect(screen.queryByLabelText("show more")).toBeNull();
  });
});

describe("DebugMessageRow — attribution + dropped sentinel", () => {
  it("renders the label, format badge, run id, and value", () => {
    const msg: DebugMessage = {
      kind: "debug",
      node: "d1",
      runId: "01JTESTRUNID",
      ts: 1700000000000,
      format: "json",
      value: { ok: true },
      label: "scaled temp",
    };
    render(<DebugMessageRow msg={msg} />);
    expect(screen.getByText("scaled temp")).toBeInTheDocument();
    expect(screen.getByText("JSON")).toBeInTheDocument();
    // The run id is truncated to its first 6 chars + … (short() in DebugMessageRow).
    expect(screen.getByText(/01JTES/)).toBeInTheDocument();
  });

  it("renders a `dropped` sentinel as 'N messages dropped' with no value body", () => {
    const msg: DebugMessage = {
      kind: "dropped",
      node: "d1",
      label: "dbg",
      dropped: 7,
    };
    const { container } = render(<DebugMessageRow msg={msg} />);
    expect(screen.getByText(/7 messages dropped/)).toBeInTheDocument();
    expect(container.querySelector('[aria-label="debug json tree"]')).toBeNull();
    expect(container.querySelector("pre")).toBeNull();
  });
});
