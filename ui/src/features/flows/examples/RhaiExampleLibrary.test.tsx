// The rhai examples library (rhai-node examples). Covers: the catalog is non-trivially populated and
// categorized; a row is a collapsible dropdown (code hidden until expanded); Use loads the body into
// the editor buffer. The Copy button's clipboard write is exercised via a stubbed `navigator.clipboard`.

import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { RhaiExampleLibrary } from "./RhaiExampleLibrary";
import { RHAI_EXAMPLE_CATEGORIES, RHAI_EXAMPLES } from "./rhaiExamples";

describe("rhai examples catalog", () => {
  it("ships ~20 examples across several categories", () => {
    expect(RHAI_EXAMPLE_CATEGORIES.length).toBeGreaterThanOrEqual(4);
    expect(RHAI_EXAMPLES.length).toBeGreaterThanOrEqual(20);
    // Every example has a non-empty title, summary, and body — no placeholder rows.
    for (const ex of RHAI_EXAMPLES) {
      expect(ex.title.length).toBeGreaterThan(0);
      expect(ex.summary.length).toBeGreaterThan(0);
      expect(ex.body.length).toBeGreaterThan(0);
    }
    // Ids are unique (the React keys + lookup depend on it).
    const ids = RHAI_EXAMPLES.map((e) => e.id);
    expect(new Set(ids).size).toBe(ids.length);
  });
});

describe("RhaiExampleLibrary", () => {
  it("hides code until a row is expanded, then reveals it as a dropdown", () => {
    render(<RhaiExampleLibrary onUse={() => {}} />);
    const first = RHAI_EXAMPLES[0];
    // Collapsed: the code block is not rendered yet.
    expect(screen.queryByLabelText(`code for ${first.title}`)).toBeNull();
    fireEvent.click(screen.getByLabelText(`toggle example ${first.title}`));
    // Expanded: the code is now visible and matches the catalog body.
    const code = screen.getByLabelText(`code for ${first.title}`);
    expect(code.textContent).toBe(first.body);
  });

  it("loads the example body into the editor when Use is clicked", () => {
    let loaded = "";
    render(<RhaiExampleLibrary onUse={(b) => (loaded = b)} />);
    const first = RHAI_EXAMPLES[0];
    fireEvent.click(screen.getByLabelText(`toggle example ${first.title}`));
    fireEvent.click(screen.getByLabelText(`use example ${first.title}`));
    expect(loaded).toBe(first.body);
  });

  it("copies the example body to the clipboard when Copy is clicked", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", { clipboard: { writeText } });
    render(<RhaiExampleLibrary onUse={() => {}} />);
    const first = RHAI_EXAMPLES[0];
    fireEvent.click(screen.getByLabelText(`toggle example ${first.title}`));
    fireEvent.click(screen.getByLabelText(`copy example ${first.title}`));
    expect(writeText).toHaveBeenCalledWith(first.body);
    vi.unstubAllGlobals();
  });
});
