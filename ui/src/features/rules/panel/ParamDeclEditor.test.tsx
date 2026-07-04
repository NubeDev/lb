// The param-declaration editor (typed params authoring). Props-driven, no I/O — verifies add/remove,
// per-field edits, that switching a param to `enum` reveals the options field, and that the options
// string parses into a trimmed, non-empty array.

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { ParamDeclEditor } from "./ParamDeclEditor";
import type { RuleParam } from "@/lib/rules";

describe("ParamDeclEditor", () => {
  it("adds a param with a text default kind", () => {
    const onChange = vi.fn();
    render(<ParamDeclEditor params={[]} onChange={onChange} />);
    fireEvent.click(screen.getByLabelText("add param"));
    expect(onChange).toHaveBeenCalledWith([{ name: "", kind: "text" }]);
  });

  it("edits a param name", () => {
    const onChange = vi.fn();
    render(<ParamDeclEditor params={[{ name: "", kind: "text" }]} onChange={onChange} />);
    fireEvent.change(screen.getByLabelText("param name 0"), { target: { value: "site" } });
    expect(onChange).toHaveBeenCalledWith([{ name: "site", kind: "text" }]);
  });

  it("removes a param", () => {
    const onChange = vi.fn();
    const params: RuleParam[] = [{ name: "a" }, { name: "b" }];
    render(<ParamDeclEditor params={params} onChange={onChange} />);
    fireEvent.click(screen.getByLabelText("remove param 0"));
    expect(onChange).toHaveBeenCalledWith([{ name: "b" }]);
  });

  it("reveals the options field only for an enum param and parses it", () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <ParamDeclEditor params={[{ name: "region", kind: "text" }]} onChange={onChange} />,
    );
    expect(screen.queryByLabelText("param options 0")).toBeNull();
    rerender(<ParamDeclEditor params={[{ name: "region", kind: "enum" }]} onChange={onChange} />);
    fireEvent.change(screen.getByLabelText("param options 0"), {
      target: { value: "emea, amer ,, apac" },
    });
    expect(onChange).toHaveBeenCalledWith([
      { name: "region", kind: "enum", options: ["emea", "amer", "apac"] },
    ]);
  });

  it("toggles required", () => {
    const onChange = vi.fn();
    render(<ParamDeclEditor params={[{ name: "site", kind: "text" }]} onChange={onChange} />);
    fireEvent.click(screen.getByLabelText("param required 0"));
    expect(onChange).toHaveBeenCalledWith([{ name: "site", kind: "text", required: true }]);
  });
});
