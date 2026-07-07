// Unit tests for MarkdownView (channels-agent scope). The renderer is pure (markdown in → React out),
// so jsdom is enough — no gateway. We assert the three render paths:
//   - ordinary markdown (headings/lists/code/etc.) renders to the right elements,
//   - a ```json fenced block parses and mounts the interactive ReactJson tree,
//   - a fenced non-json block mounts as a styled <pre><code>,
//   - inline code mounts inline (not inside a <pre>),
// plus the XSS floor: react-markdown builds vnodes (no raw HTML), so an <script> in the source is
// rendered as visible text, never as an executable element.

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { MarkdownView } from "./MarkdownView";

describe("MarkdownView — ordinary markdown", () => {
  it("renders headings, paragraphs, lists, and links", () => {
    render(
      <MarkdownView>
        {"# Title\n\nSome **bold** text with a [link](https://example.com).\n\n- one\n- two\n"}
      </MarkdownView>,
    );
    expect(screen.getByText("Title").tagName).toBe("H1");
    expect(screen.getByText("bold").tagName).toBe("STRONG");
    expect(screen.getByText("link").tagName).toBe("A");
    expect(screen.getByText("link")).toHaveAttribute("href", "https://example.com");
    expect(screen.getByText("link")).toHaveAttribute("target", "_blank");
    expect(screen.getByText("one")).toBeInTheDocument();
    expect(screen.getByText("two")).toBeInTheDocument();
  });

  it("renders a GFM table", () => {
    const md = "| a | b |\n| --- | --- |\n| 1 | 2 |\n";
    render(<MarkdownView>{md}</MarkdownView>);
    expect(screen.getByText("a")).toBeInTheDocument();
    expect(screen.getByText("b")).toBeInTheDocument();
    expect(screen.getByText("1")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
  });
});

describe("MarkdownView — fenced JSON renders the interactive tree", () => {
  it("parses a ```json object block and mounts ReactJson", () => {
    const md = "```json\n{\"name\":\"ada\",\"age\":36}\n```";
    const { container } = render(<MarkdownView>{md}</MarkdownView>);
    // The JSON tree container is labelled and mounts a nested structure with the parsed keys/values.
    expect(screen.getByLabelText("json block")).toBeInTheDocument();
    expect(container.textContent).toContain("name");
    expect(container.textContent).toContain("ada");
  });

  it("renders a ```json array block as a tree", () => {
    const md = "```json\n[1, 2, 3]\n```";
    render(<MarkdownView>{md}</MarkdownView>);
    expect(screen.getByLabelText("json block")).toBeInTheDocument();
  });

  it("falls back to a styled <pre><code> when a ```json block does not parse", () => {
    const md = "```json\n{ not valid json\n```";
    render(<MarkdownView>{md}</MarkdownView>);
    expect(screen.queryByLabelText("json block")).not.toBeInTheDocument();
    expect(document.querySelector("pre")).toBeInTheDocument();
  });
});

describe("MarkdownView — fenced non-JSON blocks and inline code", () => {
  it("renders a ```rust fenced block as <pre><code>", () => {
    const md = "```rust\nfn main() {}\n```";
    render(<MarkdownView>{md}</MarkdownView>);
    const pre = document.querySelector("pre");
    expect(pre).toBeInTheDocument();
    expect(pre?.textContent).toContain("fn main()");
  });

  it("renders inline `code` as <code>, not inside a <pre>", () => {
    render(<MarkdownView>{"Use `npm install` to add it."}</MarkdownView>);
    const code = screen.getByText("npm install");
    expect(code.tagName).toBe("CODE");
    expect(code.closest("pre")).toBeNull();
  });
});

describe("MarkdownView — XSS floor (no raw HTML reaches the DOM)", () => {
  it("renders an inline <script> as visible text, never as an executable element", () => {
    const { container } = render(
      <MarkdownView>{"<script>alert(1)</script>"}</MarkdownView>
    );
    expect(container.querySelector("script")).toBeNull();
    expect(container.textContent).toContain("<script>alert(1)</script>");
  });

  it("does not turn a markdown link into a javascript: URL", () => {
    render(<MarkdownView>{"[bad](javascript:alert(1))"}</MarkdownView>);
    const link = screen.getByText("bad");
    expect(link.tagName).toBe("A");
    expect(link.getAttribute("href")).not.toBe("javascript:alert(1)");
  });
});
