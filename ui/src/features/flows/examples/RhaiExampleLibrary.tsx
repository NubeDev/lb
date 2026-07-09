// The rhai-node examples library — a categorized, collapsible catalog rendered below the source editor
// in the node Config panel (rhai-node examples). Each row is a dropdown: click the title to reveal the
// code, then Copy it or Use it (load into the editor buffer). The catalog itself is data
// (`rhaiExamples.ts`); this file is only the presentation. Shown only for a field the descriptor marks
// as rhai (opaque `format` hint — SchemaForm decides), never by branching on a node type. One
// component per file (FILE-LAYOUT).

import { useState } from "react";
import { Check, ChevronRight, Copy, FileCode } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  RHAI_EXAMPLE_CATEGORIES,
  type RhaiExample,
} from "./rhaiExamples";

interface RhaiExampleLibraryProps {
  /** Load an example body into the editor buffer (replaces the current source). */
  onUse: (body: string) => void;
}

/** The categorized, collapsible rhai example library. */
export function RhaiExampleLibrary({ onUse }: RhaiExampleLibraryProps) {
  return (
    <div aria-label="rhai examples" className="mt-1 flex flex-col gap-2">
      <p className="text-[11px] text-muted">
        Examples — expand one to see the code, then Copy or Use it.
      </p>
      {RHAI_EXAMPLE_CATEGORIES.map((cat) => (
        <div key={cat.id} className="flex flex-col gap-1">
          <span className="px-0.5 text-[10px] font-semibold uppercase tracking-wide text-muted">
            {cat.title}
          </span>
          <ul className="flex flex-col gap-1">
            {cat.examples.map((ex) => (
              <li key={ex.id}>
                <ExampleRow example={ex} onUse={onUse} />
              </li>
            ))}
          </ul>
        </div>
      ))}
    </div>
  );
}

/** One collapsible example row: title + summary, expanding to the code with Copy + Use. */
function ExampleRow({ example, onUse }: { example: RhaiExample; onUse: (body: string) => void }) {
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState(false);

  async function copy() {
    await navigator.clipboard?.writeText(example.body);
    setCopied(true);
    // A brief "copied" confirmation, then revert to the copy affordance.
    window.setTimeout(() => setCopied(false), 1200);
  }

  return (
    <div className="overflow-hidden rounded-md border border-border bg-card">
      <button
        type="button"
        aria-label={`toggle example ${example.title}`}
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-start gap-1.5 px-2.5 py-2 text-left hover:bg-muted"
      >
        <ChevronRight
          size={13}
          className={`mt-0.5 shrink-0 text-muted transition-transform ${open ? "rotate-90" : ""}`}
          aria-hidden
        />
        <span className="flex min-w-0 flex-col gap-0.5">
          <span className="flex items-center gap-1.5 text-xs font-medium text-fg">
            <FileCode size={12} className="shrink-0 text-muted" aria-hidden />
            {example.title}
          </span>
          <span className="text-[11px] leading-snug text-muted">{example.summary}</span>
        </span>
      </button>
      {open ? (
        <div className="border-t border-border">
          <pre
            aria-label={`code for ${example.title}`}
            className="overflow-x-auto bg-bg px-2.5 py-2 font-mono text-[11px] leading-relaxed text-fg"
          >
            {example.body}
          </pre>
          <div className="flex items-center gap-2 border-t border-border px-2.5 py-1.5">
            <Button
              type="button"
              size="sm"
              variant="ghost"
              aria-label={`copy example ${example.title}`}
              onClick={copy}
              className="h-7 gap-1 px-2 text-[11px]"
            >
              {copied ? <Check size={12} className="text-accent" /> : <Copy size={12} />}
              {copied ? "Copied" : "Copy"}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="outline"
              aria-label={`use example ${example.title}`}
              onClick={() => onUse(example.body)}
              className="h-7 px-2 text-[11px]"
            >
              Use in editor
            </Button>
          </div>
        </div>
      ) : null}
    </div>
  );
}
