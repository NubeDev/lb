// The per-block "Copy code" button — the ChatGPT-style affordance that sits in a fenced code/JSON
// block's header and copies THAT block's raw source to the clipboard (channels-agent scope). Sibling of
// `AnswerCopyButton` (which copies the whole answer): this owns only the click → clipboard → transient
// "Copied" state for one block's source. One responsibility per file (FILE-LAYOUT) — the source text is
// passed in by MarkdownView's block renderers.

import { useCallback, useState } from "react";
import { Check, Copy } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface Props {
  /** The block's raw source text to copy. */
  text: string;
  className?: string;
}

export function CodeBlockCopyButton({ text, className }: Props) {
  const [copied, setCopied] = useState(false);

  const copy = useCallback(async () => {
    try {
      await navigator.clipboard?.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard denied (permissions / insecure context) — leave the button unchanged; the block text
      // is still selectable for a manual copy. Nothing destructive happened.
    }
  }, [text]);

  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      aria-label="copy code"
      title="Copy this block to the clipboard"
      onClick={() => void copy()}
      disabled={text.length === 0}
      className={cn("h-7 gap-1 px-2 text-xs text-muted hover:text-fg", className)}
    >
      {copied ? <Check size={13} className="text-accent" /> : <Copy size={13} />}
      {copied ? "Copied" : "Copy code"}
    </Button>
  );
}
