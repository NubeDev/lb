// "Copy for AI" — copy the dock session as markdown to the clipboard, so the user can paste a run to
// an external AI to help improve the agent/backend. Composition only (FILE-LAYOUT): the SHAPE lives in
// `exportTranscript` (pure, tested); this owns the click → clipboard → transient "Copied ✓" affordance.

import { useCallback, useState } from "react";
import { Check, ClipboardCopy } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { Item } from "@/lib/channel/channel.types";
import { exportTranscript, type TranscriptContext } from "./exportTranscript";

interface Props {
  ctx: TranscriptContext;
  items: Item[];
}

export function DockCopyButton({ ctx, items }: Props) {
  const [copied, setCopied] = useState(false);

  const copy = useCallback(async () => {
    const md = exportTranscript(ctx, items);
    try {
      await navigator.clipboard?.writeText(md);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard denied (permissions / insecure context) — leave the button unchanged rather than
      // erroring; the user can retry. Nothing destructive happened.
    }
  }, [ctx, items]);

  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      aria-label="copy transcript for AI"
      title="Copy this session as markdown to paste to an AI"
      onClick={() => void copy()}
      disabled={items.length === 0}
      className="h-8 w-8 shrink-0 p-0"
    >
      {copied ? <Check size={15} className="text-accent" /> : <ClipboardCopy size={15} />}
    </Button>
  );
}
