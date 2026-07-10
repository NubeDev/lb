// MarkdownEditor — a toolbar'd markdown editor with a Write/Preview toggle (reports scope;
// document-store is the named second consumer). Ported from lazybones' `markdown-editor.tsx`: a
// monospace textarea whose toolbar wraps or prefixes the current selection (bold/italic/heading/
// lists/code/link), plus a live preview rendered via `react-markdown` + `remark-gfm` (XSS-safe — raw
// HTML is escaped, no `rehype-raw`). The `value`/`onChange` contract is a MARKDOWN STRING both ways.
// `editable={false}` renders ONLY the preview (the report sheet / print view reuse it). One
// responsibility: the markdown editing surface (`MarkdownBody` owns the preview element styles).

import { useRef, useState } from "react";
import {
  Bold,
  Code,
  Eye,
  Heading,
  Italic,
  Link as LinkIcon,
  List,
  ListOrdered,
  Pencil,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { MarkdownBody } from "./MarkdownBody";

interface Props {
  /** The markdown body (both the initial value AND the shape `onChange` emits). */
  value: string;
  /** Fired with the current markdown on every edit. */
  onChange: (markdown: string) => void;
  /** Read-only skin — renders just the markdown (the live preview / print view). */
  editable?: boolean;
  /** Bare mode is implied by `editable={false}`; kept for API compatibility with callers. */
  bare?: boolean;
  /** aria-label for the editing region. */
  label?: string;
  minRows?: number;
}

export function MarkdownEditor({ value, onChange, editable = true, label = "markdown editor", minRows = 10 }: Props) {
  const ref = useRef<HTMLTextAreaElement>(null);
  const [mode, setMode] = useState<"write" | "preview">("write");

  if (!editable) {
    return <MarkdownBody label={label}>{value}</MarkdownBody>;
  }

  /** Wrap the current selection (or caret) in `before`/`after`. */
  function wrap(before: string, after = before) {
    const el = ref.current;
    if (!el) return;
    const { selectionStart: s, selectionEnd: e } = el;
    const sel = value.slice(s, e);
    onChange(value.slice(0, s) + before + sel + after + value.slice(e));
    // Restore a sensible selection around the wrapped text after React re-renders.
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(s + before.length, s + before.length + sel.length);
    });
  }

  /** Prefix each line of the current selection (or caret line) with `prefix`. */
  function prefixLines(prefix: string | ((i: number) => string)) {
    const el = ref.current;
    if (!el) return;
    const { selectionStart: s, selectionEnd: e } = el;
    const lineStart = value.lastIndexOf("\n", s - 1) + 1;
    const block = value.slice(lineStart, e);
    const prefixed = block
      .split("\n")
      .map((ln, i) => (typeof prefix === "string" ? prefix : prefix(i)) + ln)
      .join("\n");
    onChange(value.slice(0, lineStart) + prefixed + value.slice(e));
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(lineStart, lineStart + prefixed.length);
    });
  }

  const tools = [
    { icon: Heading, title: "Heading", run: () => prefixLines("## ") },
    { icon: Bold, title: "Bold", run: () => wrap("**") },
    { icon: Italic, title: "Italic", run: () => wrap("_") },
    { icon: Code, title: "Code", run: () => wrap("`") },
    { icon: List, title: "Bullet list", run: () => prefixLines("- ") },
    { icon: ListOrdered, title: "Numbered list", run: () => prefixLines((i) => `${i + 1}. `) },
    { icon: LinkIcon, title: "Link", run: () => wrap("[", "](url)") },
  ];

  return (
    <div className="rounded-md border border-border bg-panel-2/40">
      <div className="flex items-center justify-between gap-2 border-b border-border px-1.5 py-1">
        <div className={mode === "preview" ? "flex items-center gap-0.5 opacity-40" : "flex items-center gap-0.5"}>
          {tools.map((t) => (
            <Button
              key={t.title}
              type="button"
              variant="ghost"
              size="icon"
              className="h-6 w-6"
              title={t.title}
              aria-label={t.title}
              disabled={mode === "preview"}
              onClick={t.run}
            >
              <t.icon size={13} />
            </Button>
          ))}
        </div>
        <div className="flex items-center gap-0.5">
          <Button
            type="button"
            variant={mode === "write" ? "outline" : "ghost"}
            size="sm"
            className="h-6 px-2 text-[11px]"
            onClick={() => setMode("write")}
          >
            <Pencil size={11} /> Write
          </Button>
          <Button
            type="button"
            variant={mode === "preview" ? "outline" : "ghost"}
            size="sm"
            className="h-6 px-2 text-[11px]"
            onClick={() => setMode("preview")}
          >
            <Eye size={11} /> Preview
          </Button>
        </div>
      </div>

      {mode === "write" ? (
        <textarea
          ref={ref}
          aria-label={label}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder="Write markdown…"
          rows={minRows}
          className="block w-full resize-y bg-transparent px-3 py-2.5 font-mono text-xs leading-relaxed outline-none placeholder:text-muted/70"
        />
      ) : (
        <div className="min-h-[8rem] px-3 py-2.5">
          {value.trim() ? (
            <MarkdownBody>{value}</MarkdownBody>
          ) : (
            <p className="text-xs text-muted">Nothing to preview yet.</p>
          )}
        </div>
      )}
    </div>
  );
}
