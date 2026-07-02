// The in-process PROMOTION-CHECKLIST tests (genui-scope Decision 1 / promotion checklist). These are the
// CI-testable items that justify rendering the catalog IR in-process (it's trusted DATA, not code):
//   1. no catalog component uses `dangerouslySetInnerHTML` / renders a raw href/src without sanitizing;
//   2. markdown renders through a sanitizing path (no raw HTML pass-through);
//   4. every side effect goes through the leashed bridge (a control's action → `onAction`, never a
//      direct fetch/DOM escape);
//   5. CSS stays under `.gu-root` with no user-controlled class/style injection beyond token'd enums.
// (Item 3 — no prop evaluated as code — is a source-level invariant asserted in checklist.source.test.ts.)
import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "@testing-library/react";
import { GenUiSurface } from "./GenUiSurface";
import { nubeCatalog } from "../catalog/nubeCatalog";
import type { IrSpec } from "../ir/types";
import { IR_VERSION } from "../ir/types";

function surface(components: IrSpec["components"], root: string, data?: Record<string, unknown>) {
  const spec: IrSpec = { v: IR_VERSION, surface: { surfaceId: "cell", root }, components };
  return { spec, data };
}

describe("promotion checklist", () => {
  it("item 1+2: markdown does NOT pass raw HTML through (no dangerouslySetInnerHTML)", () => {
    const { spec } = surface(
      { r: { id: "r", component: "markdown", props: { value: "**bold** <img src=x onerror=alert(1)>" } } },
      "r",
    );
    const { container } = render(<GenUiSurface spec={spec} catalog={nubeCatalog} />);
    // The raw <img> must NOT become a real element — it renders as escaped text.
    expect(container.querySelector("img")).toBeNull();
    expect(container.textContent).toContain("bold");
  });

  it("item 1: a link with a non-http(s) scheme is NOT rendered as an href", () => {
    const { spec } = surface(
      { r: { id: "r", component: "markdown", props: { value: "[click](javascript:alert(1))" } } },
      "r",
    );
    const { container } = render(<GenUiSurface spec={spec} catalog={nubeCatalog} />);
    const a = container.querySelector("a");
    // Either no anchor at all, or an anchor whose href is not the javascript: URL.
    expect(a?.getAttribute("href") ?? "").not.toContain("javascript:");
  });

  it("item 4: a control's action flows through onAction (the leashed bridge), not a direct escape", () => {
    const onAction = vi.fn();
    const { spec } = surface(
      { r: { id: "r", component: "button", props: { label: "Go", value: 7 } } },
      "r",
    );
    const { getByText } = render(<GenUiSurface spec={spec} catalog={nubeCatalog} onAction={onAction} />);
    fireEvent.click(getByText("Go"));
    expect(onAction).toHaveBeenCalledTimes(1);
    expect(onAction.mock.calls[0][0]).toMatchObject({ name: "press", componentId: "r" });
  });

  it("item 5: everything renders under a single .gu-root; enum props map to fixed classes", () => {
    const { spec } = surface(
      {
        r: { id: "r", component: "stack", props: { direction: "horizontal" }, children: ["t"] },
        t: { id: "t", component: "tag", props: { text: "hot", tone: "bad" } },
      },
      "r",
    );
    const { container } = render(<GenUiSurface spec={spec} catalog={nubeCatalog} />);
    expect(container.querySelector(".gu-root")).not.toBeNull();
    expect(container.querySelector(".gu-stack.gu-horizontal")).not.toBeNull();
    // A tone enum maps to a FIXED class, never the raw value spread into className.
    expect(container.querySelector(".gu-tag.gu-tone-bad")).not.toBeNull();
  });
});

describe("render binds data via JSON Pointer", () => {
  it("resolves a $bind against the /data/{refId} model", () => {
    const { spec, data } = surface(
      { r: { id: "r", component: "stat", props: { value: { $bind: "/data/A/value" }, label: "Count" } } },
      "r",
      { data: { A: { value: 42 } } },
    );
    const { container } = render(<GenUiSurface spec={spec} data={data} catalog={nubeCatalog} />);
    expect(container.textContent).toContain("42");
    expect(container.textContent).toContain("Count");
  });

  it("an unresolvable binding renders no value, never a crash", () => {
    const { spec } = surface(
      { r: { id: "r", component: "stat", props: { value: { $bind: "/data/Z/nope" } } } },
      "r",
    );
    expect(() => render(<GenUiSurface spec={spec} catalog={nubeCatalog} />)).not.toThrow();
  });
});
