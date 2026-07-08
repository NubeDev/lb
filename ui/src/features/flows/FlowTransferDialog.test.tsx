// The export/import dialog (flow-ui-polish scope) — pretty⇄compact, the selected-nodes scope (with
// the loud stripped-wires warning), and the paste-import path: live parse feedback, the node/wire
// count on the Import button, and Apply through the caller's real save path.

import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import type { Flow } from "@/lib/flows";
import { FlowTransferDialog, type FlowTransferDialogProps } from "./FlowTransferDialog";
import { flowToJson, parseFlowJson, strippedNeedsCount } from "./flowTransfer";

const flow: Flow = {
  id: "f1",
  name: "f1",
  version: 3,
  failurePolicy: "halt",
  nodes: [
    { id: "start", type: "trigger", needs: [], config: { mode: "manual" } },
    { id: "a", type: "count", needs: ["start"], config: {} },
    { id: "b", type: "count", needs: ["a"], config: {} },
  ],
} as unknown as Flow;

function props(over: Partial<FlowTransferDialogProps> = {}): FlowTransferDialogProps {
  return {
    flow,
    selectedIds: new Set<string>(),
    open: true,
    tab: "export",
    onTabChange: vi.fn(),
    onClose: vi.fn(),
    onImport: vi.fn(async () => ({ ok: true })),
    ...over,
  };
}

describe("flowTransfer serialisation", () => {
  it("pretty vs compact round-trip the same document", () => {
    const pretty = flowToJson(flow, { pretty: true });
    const compact = flowToJson(flow, { pretty: false });
    expect(pretty).toContain("\n");
    expect(compact).not.toContain("\n");
    expect(JSON.parse(pretty)).toEqual(JSON.parse(compact));
  });

  it("a selection strips outside `needs` and counts them (never silent)", () => {
    const sel = new Set(["a", "b"]);
    expect(strippedNeedsCount(flow, sel)).toBe(1); // a ← start is outside
    const doc = JSON.parse(flowToJson(flow, { pretty: false, selection: sel })) as Flow;
    expect(doc.nodes.map((n) => n.id)).toEqual(["a", "b"]);
    expect(doc.nodes.find((n) => n.id === "a")?.needs).toEqual([]);
  });

  it("parseFlowJson pins id + workspace to the open flow and rejects non-flows", () => {
    const parsed = parseFlowJson(JSON.stringify({ ...flow, id: "other" }), flow);
    expect(parsed.id).toBe("f1");
    expect(() => parseFlowJson("{}", flow)).toThrow(/nodes/);
    expect(() => parseFlowJson("not json", flow)).toThrow();
  });
});

describe("FlowTransferDialog", () => {
  it("export preview renders the flow JSON and toggles compact", () => {
    render(<FlowTransferDialog {...props()} />);
    const preview = screen.getByLabelText("export preview");
    expect(preview.textContent).toContain('"nodes"');
    expect(preview.textContent).toContain("\n");
    fireEvent.click(screen.getByLabelText("pretty print"));
    expect(screen.getByLabelText("export preview").textContent).not.toContain("\n");
  });

  it("selected-nodes scope warns about stripped incoming wires", () => {
    render(<FlowTransferDialog {...props({ selectedIds: new Set(["a"]) })} />);
    fireEvent.click(screen.getByLabelText("selected nodes only"));
    expect(screen.getByLabelText("stripped edges warning").textContent).toContain("1 incoming wire");
    const doc = JSON.parse(screen.getByLabelText("export preview").textContent ?? "") as Flow;
    expect(doc.nodes.map((n) => n.id)).toEqual(["a"]);
  });

  it("import parses the paste live and applies through onImport", async () => {
    const onImport = vi.fn(async () => ({ ok: true }));
    const onClose = vi.fn();
    render(<FlowTransferDialog {...props({ tab: "import", onImport, onClose })} />);
    const apply = screen.getByLabelText("apply import") as HTMLButtonElement;
    expect(apply.disabled).toBe(true);

    fireEvent.change(screen.getByLabelText("import json"), {
      target: { value: "not json" },
    });
    expect(screen.getByLabelText("import parse error")).toBeTruthy();

    fireEvent.change(screen.getByLabelText("import json"), {
      target: { value: JSON.stringify(flow) },
    });
    expect(apply.textContent).toContain("3 nodes");
    fireEvent.click(apply);
    await vi.waitFor(() => expect(onImport).toHaveBeenCalledOnce());
    expect((onImport.mock.calls[0] as unknown[])[0]).toMatchObject({ id: "f1" });
    await vi.waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  });

  it("a host reject renders inline and keeps the dialog open", async () => {
    const onImport = vi.fn(async () => ({ ok: false, error: "cycle detected" }));
    const onClose = vi.fn();
    render(<FlowTransferDialog {...props({ tab: "import", onImport, onClose })} />);
    fireEvent.change(screen.getByLabelText("import json"), {
      target: { value: JSON.stringify(flow) },
    });
    fireEvent.click(screen.getByLabelText("apply import"));
    await vi.waitFor(() =>
      expect(screen.getByLabelText("import error").textContent).toContain("cycle detected"),
    );
    expect(onClose).not.toHaveBeenCalled();
  });
});
