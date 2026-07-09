// The icon picker: a search box over the whole lucide catalog with a paged grid, so a
// developer/author can browse ~1500 glyphs without rendering them all at once. `pageSize`
// is adjustable (default 10) — the grid shows that many, "Show more" reveals another page.
// The selected name is stored as opaque data (see `resolveIcon`); this component only
// picks it. Pair with a Dialog/Popover trigger at the call site. One responsibility per
// file (FILE-LAYOUT): the picker body — the resolver, catalog and <Icon> are separate.

import * as React from "react";
import { Search } from "lucide-react";

import { cn } from "@/lib/utils";
import { Input } from "@/components/ui/input";
import { searchIcons, type IconEntry } from "./catalog";

export interface IconPickerProps {
  /** Currently selected icon name (kebab-case), if any. */
  value?: string | null;
  onSelect: (name: string) => void;
  /** Icons revealed per page. Adjustable; defaults to 10. */
  pageSize?: number;
  /** Grid columns. Defaults to 5. */
  columns?: number;
  className?: string;
  /** Autofocus the search input on mount. */
  autoFocus?: boolean;
}

export function IconPicker({
  value,
  onSelect,
  pageSize = 10,
  columns = 5,
  className,
  autoFocus,
}: IconPickerProps) {
  const [query, setQuery] = React.useState("");
  const [shown, setShown] = React.useState(pageSize);

  // Reset the page window whenever the query (or the page size) changes.
  React.useEffect(() => setShown(pageSize), [query, pageSize]);

  const matches = React.useMemo(() => searchIcons(query, 500), [query]);
  const page: IconEntry[] = matches.slice(0, shown);
  const hasMore = matches.length > shown;

  return (
    <div className={cn("flex flex-col gap-3", className)}>
      <div className="relative">
        <Search className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-fg/50" />
        <Input
          autoFocus={autoFocus}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search icons…"
          aria-label="Search icons"
          className="pl-8"
        />
      </div>

      {page.length === 0 ? (
        <p className="py-6 text-center text-sm text-fg/50">No icons match “{query}”.</p>
      ) : (
        <div
          role="listbox"
          aria-label="Icons"
          className="grid gap-1.5"
          style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}
        >
          {page.map(({ name, Icon }) => {
            const selected = name === value;
            return (
              <button
                key={name}
                type="button"
                role="option"
                aria-selected={selected}
                title={name}
                onClick={() => onSelect(name)}
                className={cn(
                  "flex aspect-square items-center justify-center rounded-md border border-transparent",
                  "text-fg/80 transition-colors hover:bg-panel-2 hover:text-fg",
                  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
                  selected && "border-accent bg-accent/10 text-accent",
                )}
              >
                <Icon className="size-5" aria-hidden />
              </button>
            );
          })}
        </div>
      )}

      <div className="flex items-center justify-between text-xs text-fg/50">
        <span>
          {Math.min(shown, matches.length)} of {matches.length}
        </span>
        {hasMore && (
          <button
            type="button"
            onClick={() => setShown((n) => n + pageSize)}
            className="rounded px-2 py-1 font-medium text-accent hover:bg-panel-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            Show more
          </button>
        )}
      </div>
    </div>
  );
}
