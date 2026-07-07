// The "Copy AI prompt" control for the template editor (template-prompt slice) — a sample-size pick
// (10 / 25 / all rows) + one copy button that puts `buildTemplatePrompt(rows, sample)` on the
// clipboard: the engine contract AND the draft's real data in a single paste, so the user can ask any
// LLM for a widget and paste the returned HTML straight back into the inline editor. Rows come from
// `useResultRows` (the same frames the preview renders — no second fetch). One responsibility: the
// copy affordance; the prompt text is `templatePrompt.ts`.

import { useState } from "react";
import { Sparkles } from "lucide-react";

import { Button } from "@/components/ui/button";

import { useResultRows } from "./fields/RowsContext";
import { buildTemplatePrompt, type PromptQuery, type PromptSample } from "./templatePrompt";

const SAMPLES: { value: PromptSample; label: string }[] = [
  { value: 10, label: "10 sample rows" },
  { value: 25, label: "25 sample rows" },
  { value: "all", label: "all rows" },
];

export function CopyTemplatePrompt({ query }: { query?: PromptQuery }) {
  const rows = useResultRows();
  const [sample, setSample] = useState<PromptSample>(10);
  const [copied, setCopied] = useState(false);

  const copy = () => {
    void navigator.clipboard?.writeText(buildTemplatePrompt(rows, sample, query));
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1500);
  };

  return (
    <div className="flex items-center gap-2 text-xs">
      <Button aria-label="copy ai prompt" size="sm" variant="outline" className="h-6 gap-1 px-2" onClick={copy}>
        <Sparkles size={11} /> {copied ? "Copied" : "Copy AI prompt"}
      </Button>
      <span className="text-muted">with</span>
      {/* eslint-disable-next-line no-restricted-syntax -- matches the builder's plain <select> fields */}
      <select
        aria-label="prompt data sample"
        className="h-6 rounded-md border border-border bg-bg px-1.5 text-xs text-fg"
        value={String(sample)}
        onChange={(e) => setSample(e.target.value === "all" ? "all" : Number(e.target.value))}
      >
        {SAMPLES.map((s) => (
          <option key={String(s.value)} value={String(s.value)}>
            {s.label}
          </option>
        ))}
      </select>
      <span className="text-muted">— paste the reply into the editor below.</span>
    </div>
  );
}
