// The searchable Select primitive (editor-parity scope, step 1) — the shadcn-combobox pattern bound to
// the Lazybones tokens, WITHOUT the Radix/cmdk dependency (same policy as `select.tsx`): a trigger
// button that opens an inline type-to-filter listbox. Supports grouped options, one-line descriptions,
// an optional color swatch per option, and `allowCustom` (the typed text commits as the value — the
// degrade path for a field picker with no preview frames yet). One primitive per file (FILE-LAYOUT).

import * as React from "react";
import { Check, ChevronsUpDown } from "lucide-react";

import { cn } from "@/lib/utils";

export interface ComboboxOption {
  value: string;
  /** Display label (defaults to the value). */
  label?: string;
  /** Optional group header this option renders under. */
  group?: string;
  /** Optional one-line description rendered under the label. */
  description?: string;
  /** Optional CSS color rendered as a swatch dot before the label. */
  swatch?: string;
}

interface ComboboxProps {
  options: ComboboxOption[];
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  /** The accessible name of the trigger (and `<name> search` for the filter input). */
  "aria-label": string;
  /** Commit free-typed text as the value on Enter (the labeled escape hatch, e.g. no frames yet). */
  allowCustom?: boolean;
  disabled?: boolean;
  className?: string;
}

/** Case-insensitive filter over value + label + description. */
function matches(o: ComboboxOption, q: string): boolean {
  if (!q) return true;
  const needle = q.toLowerCase();
  return (
    o.value.toLowerCase().includes(needle) ||
    (o.label ?? "").toLowerCase().includes(needle) ||
    (o.description ?? "").toLowerCase().includes(needle)
  );
}

export function Combobox({
  options,
  value,
  onChange,
  placeholder = "— select —",
  "aria-label": ariaLabel,
  allowCustom = false,
  disabled = false,
  className,
}: ComboboxProps) {
  const [open, setOpen] = React.useState(false);
  const [query, setQuery] = React.useState("");
  const [active, setActive] = React.useState(0);
  const rootRef = React.useRef<HTMLDivElement>(null);
  const inputRef = React.useRef<HTMLInputElement>(null);

  const filtered = options.filter((o) => matches(o, query));
  const current = options.find((o) => o.value === value);

  // Close on an outside pointerdown (no portal — the list renders inline, absolutely positioned).
  React.useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", onDown);
    return () => document.removeEventListener("pointerdown", onDown);
  }, [open]);

  React.useEffect(() => {
    if (open) {
      setQuery("");
      setActive(0);
      // focus the filter input once the list is on screen
      queueMicrotask(() => inputRef.current?.focus());
    }
  }, [open]);

  const commit = (v: string) => {
    onChange(v);
    setOpen(false);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      setOpen(false);
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((i) => Math.min(i + 1, filtered.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const hit = filtered[active];
      if (hit) commit(hit.value);
      else if (allowCustom && query.trim()) commit(query.trim());
    }
  };

  // Render options grouped in first-seen group order; ungrouped options come first.
  const groups: Array<{ name: string | undefined; items: Array<{ o: ComboboxOption; idx: number }> }> = [];
  filtered.forEach((o, idx) => {
    const g = groups.find((x) => x.name === o.group);
    if (g) g.items.push({ o, idx });
    else groups.push({ name: o.group, items: [{ o, idx }] });
  });

  return (
    <div ref={rootRef} className={cn("relative", className)}>
      <button
        type="button"
        role="combobox"
        aria-expanded={open}
        aria-label={ariaLabel}
        disabled={disabled}
        data-slot="combobox-trigger"
        className="flex h-8 w-full items-center justify-between gap-1 rounded-md border border-border bg-bg px-2.5 text-left text-xs text-fg transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/30 disabled:cursor-not-allowed disabled:opacity-50"
        onClick={() => setOpen((o) => !o)}
      >
        <span className={cn("flex min-w-0 items-center gap-1.5 truncate", !current && "text-muted")}>
          {current?.swatch && (
            <span className="inline-block h-3 w-3 shrink-0 rounded-full" style={{ background: current.swatch }} aria-hidden />
          )}
          {current ? (current.label ?? current.value) : value || placeholder}
        </span>
        <ChevronsUpDown size={12} className="shrink-0 text-muted" aria-hidden />
      </button>

      {open && (
        <div
          data-slot="combobox-list"
          className="absolute z-50 mt-1 max-h-64 w-full min-w-48 overflow-y-auto rounded-md border border-border bg-panel p-1 shadow-md"
        >
          <input
            ref={inputRef}
            aria-label={`${ariaLabel} search`}
            className="mb-1 h-7 w-full rounded border border-border bg-bg px-2 text-xs text-fg focus-visible:outline-none"
            placeholder={allowCustom ? "search or type…" : "search…"}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setActive(0);
            }}
            onKeyDown={onKeyDown}
          />
          <ul role="listbox" aria-label={`${ariaLabel} options`}>
            {filtered.length === 0 && !allowCustom && (
              <li className="px-2 py-1.5 text-xs text-muted">no matches</li>
            )}
            {filtered.length === 0 && allowCustom && query.trim() && (
              <li>
                <button
                  type="button"
                  className="w-full rounded px-2 py-1.5 text-left text-xs text-fg hover:bg-bg"
                  onClick={() => commit(query.trim())}
                >
                  Use “{query.trim()}”
                </button>
              </li>
            )}
            {groups.map((g) => (
              <React.Fragment key={g.name ?? "__ungrouped__"}>
                {g.name && (
                  <li className="px-2 pb-0.5 pt-1.5 text-[10px] font-medium uppercase tracking-wide text-muted" aria-hidden>
                    {g.name}
                  </li>
                )}
                {g.items.map(({ o, idx }) => (
                  <li key={o.value}>
                    <button
                      type="button"
                      role="option"
                      aria-selected={o.value === value}
                      className={cn(
                        "flex w-full items-start gap-1.5 rounded px-2 py-1.5 text-left text-xs text-fg hover:bg-bg",
                        idx === active && "bg-bg",
                      )}
                      onClick={() => commit(o.value)}
                    >
                      {o.swatch && (
                        <span className="mt-0.5 inline-block h-3 w-3 shrink-0 rounded-full" style={{ background: o.swatch }} aria-hidden />
                      )}
                      <span className="min-w-0 flex-1">
                        <span className="block truncate">{o.label ?? o.value}</span>
                        {o.description && <span className="block truncate text-[10px] text-muted">{o.description}</span>}
                      </span>
                      {o.value === value && <Check size={12} className="mt-0.5 shrink-0 text-accent" aria-hidden />}
                    </button>
                  </li>
                ))}
              </React.Fragment>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
