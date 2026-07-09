// The one shared "view / copy / download JSON" popout (ui-standards). A reusable modal that shows a
// formatted JSON (or any text) payload in a scrollable, monospace, selectable block with a Copy button
// and an optional Download button. Any surface that produces an inspectable payload — a dashboard/widget
// export, a data-inspector dump, a config blob — mounts THIS instead of a bespoke modal + copy button.
// One responsibility per file (FILE-LAYOUT): the popout chrome + the copy/download affordances. The
// caller owns the payload (pass either `json` to pretty-print, or a ready `text`).

import type { ReactNode } from "react";
import { Download } from "lucide-react";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { CopyButton } from "@/components/ui/copy-button";
import { downloadText } from "@/lib/download";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Modal title (e.g. "Export dashboard"). */
  title: string;
  /** Optional one-line description under the title. */
  description?: ReactNode;
  /** A value to pretty-print as JSON. Ignored when `text` is provided. */
  json?: unknown;
  /** A ready-made text payload (overrides `json`). Use when you already have the exact bytes. */
  text?: string;
  /** Copy-button label. Defaults to "Copy JSON". */
  copyLabel?: string;
  /** When set, a Download button writes the payload to this filename. Omit ⇒ no download button. */
  downloadName?: string;
  /** Extra footer controls (left of Close), e.g. an "Open in editor" action. */
  extraActions?: ReactNode;
}

/** Pretty-print a value as JSON, falling back to a String() for anything non-serializable (a cyclic
 *  object, a BigInt) so the popout always shows *something* rather than throwing. */
function toText(json: unknown, text?: string): string {
  if (typeof text === "string") return text;
  try {
    return JSON.stringify(json, null, 2);
  } catch {
    return String(json);
  }
}

export function JsonPopout({
  open,
  onOpenChange,
  title,
  description,
  json,
  text,
  copyLabel = "Copy JSON",
  downloadName,
  extraActions,
}: Props) {
  const payload = toText(json, text);
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          {description && <DialogDescription>{description}</DialogDescription>}
        </DialogHeader>

        <div className="relative">
          <div className="absolute right-2 top-2 z-10">
            <CopyButton text={payload} label={copyLabel} variant="outline" />
          </div>
          <pre
            aria-label="json payload"
            className="max-h-[55vh] overflow-auto rounded-md border border-border bg-panel-2/50 p-3 pr-24 font-mono text-xs leading-5 text-fg"
          >
            {payload}
          </pre>
        </div>

        <DialogFooter>
          {extraActions}
          {downloadName && (
            <Button
              type="button"
              variant="outline"
              onClick={() => downloadText(downloadName, payload)}
            >
              <Download size={13} /> Download
            </Button>
          )}
          <Button type="button" onClick={() => onOpenChange(false)}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
