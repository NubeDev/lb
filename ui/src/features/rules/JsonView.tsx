// JsonView — the raw-JSON view of a run result (rules-editor-ux scope). The workbench's result region
// toggles between the typed views (table / scalar / findings) and this: the verbatim `RunResult` as
// pretty-printed JSON, so an author can see the exact shape a rule returned (keys, nesting, the row
// shape that differs platform vs. federation) and copy it. Faithful, never abridged — the product
// principle "make raw data humane, keep values faithful". One component per file (FILE-LAYOUT).

import { useState } from "react";
import { Check, Copy } from "lucide-react";

import type { RunResult } from "@/lib/rules";

interface JsonViewProps {
  result: RunResult;
}

export function JsonView({ result }: JsonViewProps) {
  const [copied, setCopied] = useState(false);
  const text = JSON.stringify(result, null, 2);

  async function copy() {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1200);
    } catch {
      // Clipboard denied (insecure context / permissions) — the visible JSON is still selectable.
    }
  }

  return (
    <div aria-label="json result" className="relative">
      <button
        type="button"
        aria-label="copy json"
        onClick={() => void copy()}
        className="absolute right-2 top-2 inline-flex items-center gap-1 rounded-md border border-border bg-bg/80 px-2 py-1 text-xs text-muted backdrop-blur transition-colors hover:bg-panel hover:text-fg"
      >
        {copied ? <Check size={12} /> : <Copy size={12} />}
        {copied ? "Copied" : "Copy"}
      </button>
      <pre className="overflow-auto rounded-md border border-border bg-bg p-3 font-mono text-[12.5px] leading-relaxed text-fg">
        {text}
      </pre>
    </div>
  );
}
