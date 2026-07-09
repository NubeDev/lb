// Copy the agent's durable answer text to the clipboard — the per-message "Copy answer" affordance on
// the `agent_result` card (channels-agent scope). Sibling of `DockCopyButton` (which copies the whole
// transcript as markdown): this owns only the click → clipboard → transient "Copied ✓" affordance for a
// single answer string. One responsibility per file (FILE-LAYOUT) — presentation + the copied state;
// the text it copies is passed in by the caller.

import { useCallback, useState } from "react";
import { Check, ClipboardCopy } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface Props {
  /** The agent's answer text to copy. */
  text: string;
  className?: string;
}

export function AnswerCopyButton({ text, className }: Props) {
  const [copied, setCopied] = useState(false);

  const copy = useCallback(async () => {
    try {
      await navigator.clipboard?.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard denied (permissions / insecure context) — leave the button unchanged rather than
      // erroring; the answer is still selectable for a manual copy. Nothing destructive happened.
    }
  }, [text]);

  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      aria-label="copy agent answer"
      title="Copy the agent's answer to the clipboard"
      onClick={() => void copy()}
      disabled={text.length === 0}
      className={cn(
        "h-7 w-7 shrink-0 p-0 text-muted opacity-0 transition-opacity hover:text-fg group-hover:opacity-100 group-focus-within:opacity-100",
        className,
      )}
    >
      {copied ? <Check size={14} className="text-accent" /> : <ClipboardCopy size={14} />}
    </Button>
  );
}
