// The per-icon color picker — a whole-row swatch trigger opening an in-DOM popover with the full
// 100-color palette grid + a custom-hex field. NOT a native `<input type="color">` (silent no-op on
// WebKitGTK — the Tauri Linux webview — per the appearance-scope's documented bug). One surface's
// picker is one component, so the IconColors section can render a flat list of these.
// One component per file (FILE-LAYOUT).

import * as React from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ICON_COLOR_PALETTE, isValidHex, useTheme } from "@/lib/theme";
import { cn } from "@/lib/utils";

interface Props {
  /** The surface id this picker edits (`channels`, `dashboards`, …). */
  surface: string;
  /** Human label for the row. */
  label: string;
  /** Current color (`#rrggbb`) or undefined when the surface has no override. */
  value: string | undefined;
  /** Whether the picker offers "clear" (remove the override). Hidden when colorization is barely on. */
  onClear?: () => void;
}

export function IconColorSwatch({ surface, label, value, onClear }: Props) {
  const { setIconColor } = useTheme();
  const [open, setOpen] = React.useState(false);
  const rootRef = React.useRef<HTMLDivElement>(null);

  // Outside-click / Escape dismissal — the popover is in-DOM, so we own its dismissal.
  React.useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const pick = (color: string) => {
    setIconColor(surface, color);
    setOpen(false);
  };

  const onHex = (raw: string) => {
    const v = raw.trim().startsWith("#") ? raw.trim() : `#${raw.trim()}`;
    if (isValidHex(v)) pick(v.toLowerCase());
  };

  return (
    <div ref={rootRef} className="relative">
      <Button
        type="button"
        variant="ghost"
        aria-label={`${label} icon color${value ? `: ${value}` : ": default"}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        className="h-auto w-full justify-between px-1 py-1 font-normal"
      >
        <span className="text-xs text-fg">{label}</span>
        <span className="flex items-center gap-1.5">
          <span className="font-mono text-[11px] text-muted">{value ?? "default"}</span>
          <span
            aria-hidden
            className="h-4 w-6 rounded-sm border border-border"
            style={{ backgroundColor: value ?? "transparent" }}
          />
        </span>
      </Button>

      {open && (
        <div
          role="dialog"
          aria-label={`${label} icon color`}
          className="absolute right-0 z-50 mt-1 w-60 space-y-2.5 rounded-md border border-border bg-panel p-3 shadow-lg"
        >
          <div className="grid grid-cols-10 gap-1" role="group" aria-label="Color palette">
            {ICON_COLOR_PALETTE.map((c) => (
              <Button
                key={c}
                type="button"
                variant="ghost"
                aria-label={c}
                aria-pressed={value?.toLowerCase() === c}
                title={c}
                onClick={() => pick(c)}
                className={cn(
                  "h-4 w-4 rounded-sm border p-0 transition-transform hover:scale-110",
                  value?.toLowerCase() === c ? "border-fg" : "border-transparent",
                )}
                style={{ backgroundColor: c }}
              />
            ))}
          </div>

          <label className="flex items-center gap-2">
            <span className="text-[11px] text-muted">Hex</span>
            <Input
              type="text"
              aria-label={`${label} custom hex`}
              placeholder="#rrggbb"
              defaultValue={value ?? ""}
              key={value}
              onBlur={(e) => onHex(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && onHex((e.target as HTMLInputElement).value)}
              spellCheck={false}
              className="h-7 font-mono text-xs"
            />
          </label>

          {value && onClear && (
            <Button
              type="button"
              variant="ghost"
              onClick={() => {
                onClear();
                setOpen(false);
              }}
              className="h-auto w-full py-1 text-[11px] font-normal text-muted"
            >
              Reset to default
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
