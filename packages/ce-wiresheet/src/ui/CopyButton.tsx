// Small copy-to-clipboard button for the string read/edit popups and action
// return values — long strings are awkward to select out of a fixed-size popup,
// so one click grabs the whole value. Briefly flips to a "Copied" check on
// success. `compact` renders icon-only (for tight rows).

import { useEffect, useRef, useState } from "react";
import { Copy, Check } from "lucide-react";

export function CopyButton({
  text,
  title = "Copy to clipboard",
  compact = false,
}: {
  text: string;
  title?: string;
  compact?: boolean;
}) {
  const [done, setDone] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => { if (timer.current) clearTimeout(timer.current); }, []);

  const copy = (e: React.MouseEvent) => {
    e.stopPropagation();
    void navigator.clipboard?.writeText(text).then(
      () => {
        setDone(true);
        if (timer.current) clearTimeout(timer.current);
        timer.current = setTimeout(() => setDone(false), 1200);
      },
      () => {},
    );
  };

  return (
    <button
      type="button"
      onClick={copy}
      title={title}
      className="nodrag"
      style={{
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        gap: 4,
        padding: compact ? 3 : "3px 7px",
        fontSize: 11,
        lineHeight: 1,
        color: done ? "hsl(var(--green))" : "hsl(var(--muted-foreground))",
        background: "hsl(var(--secondary))",
        border: "1px solid hsl(var(--border))",
        borderRadius: 4,
        cursor: "pointer",
        flexShrink: 0,
      }}
    >
      {done ? <Check size={12} /> : <Copy size={12} />}
      {!compact && (done ? "Copied" : "Copy")}
    </button>
  );
}
