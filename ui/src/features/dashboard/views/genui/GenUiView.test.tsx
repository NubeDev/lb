// GenUiView draft/invalid classification (no gateway — these short-circuit before any data hook, so a
// pure render is enough). Regression for "adding a widget shows 'invalid genui widget (no IR)'": an
// un-authored genui cell must render an author-me DRAFT placeholder, not an error.
import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import type { Cell } from "@/lib/dashboard";
import { GenUiView } from "./GenUiView";

const base: Cell = {
  i: "g1", x: 0, y: 0, w: 6, h: 4, v: 3, widget_type: "chart", view: "genui", binding: { series: "" },
};

describe("GenUiView draft/invalid", () => {
  it("un-authored (no options.genui) → author-me draft placeholder, NOT an error", () => {
    const { container } = render(<GenUiView cell={base} />);
    expect(container.textContent).toMatch(/AI widget/i);
    expect(container.textContent).not.toMatch(/invalid/i);
  });

  it("genui block with no ir yet → still a draft, not invalid", () => {
    const cell: Cell = { ...base, options: { genui: { v: 1 } } };
    const { container } = render(<GenUiView cell={cell} />);
    expect(container.textContent).toMatch(/AI widget/i);
    expect(container.textContent).not.toMatch(/invalid/i);
  });

  it("a present-but-malformed ir → invalid (defense-in-depth)", () => {
    const cell: Cell = { ...base, options: { genui: { v: 1, ir: "not-an-object" } } };
    const { container } = render(<GenUiView cell={cell} />);
    expect(container.textContent).toMatch(/invalid genui/i);
  });

  it("a well-formed ir renders the surface", () => {
    const cell: Cell = {
      ...base,
      options: {
        genui: {
          v: 1,
          ir: {
            v: 1,
            surface: { surfaceId: "cell", root: "r" },
            components: { r: { id: "r", component: "text", props: { value: "hello" } } },
          },
        },
      },
    };
    const { container } = render(<GenUiView cell={cell} />);
    expect(container.querySelector(".gu-root")).not.toBeNull();
    expect(container.textContent).toContain("hello");
  });
});
