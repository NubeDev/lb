// Unit tests for the shared JSON popout + its copy affordance (ui-standards). jsdom has no real
// clipboard, so we stub `navigator.clipboard.writeText` — a test-only shim of a BROWSER API (not a fake
// backend; CLAUDE §9 is about node behavior). Asserts: the payload renders as pretty JSON, Copy writes
// the exact bytes and flips to "Copied", and a non-serializable value degrades instead of throwing.

import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { JsonPopout } from "./json-popout";

// A stubbed browser clipboard (jsdom has none). `writeText` records the last write so a test can assert
// the exact bytes copied. This is a test-only shim of a BROWSER API, not a fake backend (CLAUDE §9).
const writeText = vi.fn<(t: string) => Promise<void>>().mockResolvedValue(undefined);

beforeEach(() => {
  writeText.mockClear();
  vi.stubGlobal("navigator", { clipboard: { writeText } });
});

describe("JsonPopout", () => {
  it("pretty-prints the payload and copies the exact JSON", async () => {
    const value = { kind: "demo", n: 1, nested: { a: [1, 2] } };
    render(
      <JsonPopout open onOpenChange={() => {}} title="Export" json={value} />,
    );

    const pre = screen.getByLabelText("json payload");
    expect(pre.textContent).toBe(JSON.stringify(value, null, 2));

    await userEvent.click(screen.getByRole("button", { name: /copy json/i }));
    expect(writeText).toHaveBeenCalledWith(JSON.stringify(value, null, 2));
    await waitFor(() => expect(screen.getByText("Copied")).toBeInTheDocument());
  });

  it("prefers an explicit `text` payload over `json`", () => {
    render(
      <JsonPopout
        open
        onOpenChange={() => {}}
        title="Raw"
        json={{ a: 1 }}
        text="literal bytes"
      />,
    );
    expect(screen.getByLabelText("json payload").textContent).toBe(
      "literal bytes",
    );
  });

  it("degrades a non-serializable value instead of throwing", () => {
    const cyclic: Record<string, unknown> = {};
    cyclic.self = cyclic;
    // Should render *something* (the String() fallback), not crash.
    render(
      <JsonPopout open onOpenChange={() => {}} title="Cyclic" json={cyclic} />,
    );
    expect(screen.getByLabelText("json payload").textContent).toBeTruthy();
  });
});
