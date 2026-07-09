// The one shared "copy to clipboard" hook (used by `CopyButton`, the JSON popout, and any surface that
// needs a copy affordance). Wraps `navigator.clipboard.writeText` with a transient `copied` flag that
// self-resets, and a graceful `error` when the clipboard is denied (insecure context / permissions) —
// callers keep the text selectable for a manual copy rather than pretending it worked. One
// responsibility per file (FILE-LAYOUT). No React tree, no DOM assumptions beyond `navigator`.

import { useCallback, useEffect, useRef, useState } from "react";

export interface Clipboard {
  /** True for ~`resetMs` after a successful copy (drives the "Copied" affordance). */
  copied: boolean;
  /** Set when the last copy failed (denied / no clipboard); cleared on the next attempt. */
  error: string | null;
  /** Copy `text`; resolves to whether it succeeded. Never throws. */
  copy: (text: string) => Promise<boolean>;
}

export function useClipboard(resetMs = 1500): Clipboard {
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Clear the pending reset on unmount so we never setState on a gone component.
  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
    },
    [],
  );

  const copy = useCallback(
    async (text: string): Promise<boolean> => {
      try {
        if (!navigator.clipboard?.writeText)
          throw new Error("Clipboard unavailable");
        await navigator.clipboard.writeText(text);
        setError(null);
        setCopied(true);
        if (timer.current) clearTimeout(timer.current);
        timer.current = setTimeout(() => setCopied(false), resetMs);
        return true;
      } catch (e) {
        setCopied(false);
        setError(e instanceof Error ? e.message : "Copy failed");
        return false;
      }
    },
    [resetMs],
  );

  return { copied, error, copy };
}
