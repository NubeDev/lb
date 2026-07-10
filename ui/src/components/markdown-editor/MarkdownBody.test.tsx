// MarkdownBody unit coverage (reports scope): the read-only markdown renderer renders the GFM surface
// the reports A4 preview + the editor's Preview pane use (headings, paragraphs, lists, tables, code,
// links). XSS-safe by construction (react-markdown escapes raw HTML — no rehype-raw), so a raw-HTML
// block must render as literal text, not execute/parse. One test per element kind the report sheet
// actually emits; the `data-testid` anchors the snapshot the editor's preview pane reuses.

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { MarkdownBody } from "./MarkdownBody";

describe("MarkdownBody", () => {
  it("renders the container with its data-testid", () => {
    const { container } = render(<MarkdownBody>{"# Hi"}</MarkdownBody>);
    expect(screen.getByTestId("markdown-body")).toBeInTheDocument();
    // ReactMarkdown emits the wrapper div + the heading element.
    expect(container.querySelector("h1")).toBeInTheDocument();
  });

  it("renders headings, a paragraph, and emphasis", () => {
    render(
      <MarkdownBody>{"# Title\n\nA paragraph with **bold** and _italic_.\n\n## Sub"}</MarkdownBody>,
    );
    expect(screen.getByText("Title").tagName).toBe("H1");
    expect(screen.getByText("Sub").tagName).toBe("H2");
    expect(screen.getByText("bold").tagName).toBe("STRONG");
    expect(screen.getByText("italic").tagName).toBe("EM");
    expect(screen.getByText(/A paragraph with/)).toBeInTheDocument();
  });

  it("renders a GFM table with header + body cells", () => {
    const md = `| Site | kWh |\n| --- | ---: |\n| HQ | 100 |`;
    const { container } = render(<MarkdownBody>{md}</MarkdownBody>);
    const table = container.querySelector("table");
    expect(table).toBeInTheDocument();
    expect(screen.getByText("Site")).toBeInTheDocument();
    expect(screen.getByText("kWh")).toBeInTheDocument();
    expect(screen.getByText("100")).toBeInTheDocument();
  });

  it("renders bullet + numbered lists", () => {
    const { container } = render(
      <MarkdownBody>{"- one\n- two\n\n1. first\n2. second"}</MarkdownBody>,
    );
    expect(container.querySelector("ul")).toBeInTheDocument();
    expect(container.querySelector("ol")).toBeInTheDocument();
    expect(screen.getByText("one")).toBeInTheDocument();
    expect(screen.getByText("first")).toBeInTheDocument();
  });

  it("renders inline + fenced code", () => {
    const { container } = render(
      <MarkdownBody>{"Use `cmd` for that.\n\n```\nx = 1\n```"}</MarkdownBody>,
    );
    const codes = container.querySelectorAll("code");
    expect(codes.length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("cmd")).toBeInTheDocument();
  });

  it("escapes raw HTML (XSS-safe — no rehype-raw)", () => {
    // The payload is built from pieces so the literal `</script>` never appears in JSX text (which
    // the TS/JSX parser would mis-handle). react-markdown escapes raw HTML to a literal text node.
    const payload = "<scr" + "ipt>alert(1)</scr" + "ipt>";
    render(<MarkdownBody>{payload}</MarkdownBody>);
    expect(document.querySelector("script")).not.toBeInTheDocument();
  });
});
