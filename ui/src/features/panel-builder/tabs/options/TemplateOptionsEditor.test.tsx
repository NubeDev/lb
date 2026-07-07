// Regression for the "Template card renders 'no template'" bug (builder-ergonomics session): a
// freshly-picked template cell has neither `options.code` nor `options.templateId`, and the shipped
// starter was only inserted by a redundant click on the already-active Inline tab — the editor sat
// empty and the preview showed nothing. The editor must SEED the starter once, and must never
// overwrite code the user set (including a deliberately cleared "").

import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { cellToEditorState } from "@/lib/panel-kit/cellEditorState";
import type { Cell } from "@/lib/dashboard";
import { DEFAULT_INLINE_CODE } from "@/features/dashboard/builder/editors/TemplateSourceField";

import { TemplateOptionsEditor } from "./TemplateOptionsEditor";

const cell = (options: Record<string, unknown>): Cell =>
  ({ i: "t1", view: "template", title: "", options }) as unknown as Cell;

describe("TemplateOptionsEditor", () => {
  it("seeds the shipped starter when the cell has no code and no templateId", async () => {
    const patch = vi.fn();
    render(<TemplateOptionsEditor state={cellToEditorState(cell({}))} patch={patch} />);
    await waitFor(() => expect(patch).toHaveBeenCalled());
    const carry = patch.mock.calls[0][0].carry;
    expect(carry.extraOptions.code).toBe(DEFAULT_INLINE_CODE);
  });

  it("does NOT overwrite existing inline code — even an emptied editor", async () => {
    const patch = vi.fn();
    render(<TemplateOptionsEditor state={cellToEditorState(cell({ code: "" }))} patch={patch} />);
    await screen.findByLabelText("template code");
    expect(patch).not.toHaveBeenCalled();
  });

  it("does NOT seed when a saved template is referenced", async () => {
    const patch = vi.fn();
    render(
      <TemplateOptionsEditor state={cellToEditorState(cell({ templateId: "render_template:x" }))} patch={patch} />,
    );
    await screen.findByLabelText("saved template");
    expect(patch).not.toHaveBeenCalled();
  });
});
