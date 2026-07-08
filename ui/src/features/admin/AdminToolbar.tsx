// The one toolbar every admin list tab wears (access-console consistency): a thin bar with a
// left-aligned search filter and a right-aligned action slot (the "New …" button), so People,
// Roles, Workspaces, and API Keys all read the same. Replaces the three ad-hoc placements the
// tabs grew independently. Markup only — the filter value is owned by the caller. One
// responsibility per file (FILE-LAYOUT).

import type { ReactNode } from "react";
import { Search } from "lucide-react";

import { Input } from "@/components/ui/input";

interface Props {
  /** Current filter text (controlled by the tab). Omit `onSearch` to hide the search box. */
  search?: string;
  onSearch?: (value: string) => void;
  searchPlaceholder?: string;
  /** Right-aligned control — typically the "New …" `Button`. */
  action?: ReactNode;
}

export function AdminToolbar({ search, onSearch, searchPlaceholder = "Filter…", action }: Props) {
  return (
    <div className="flex min-h-12 items-center gap-2 border-b border-border bg-panel px-3 py-2">
      {onSearch && (
        <div className="relative min-w-0 flex-1 sm:max-w-xs">
          <Search
            size={13}
            className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"
          />
          <Input
            aria-label="filter"
            className="h-8 pl-8"
            placeholder={searchPlaceholder}
            value={search ?? ""}
            onChange={(e) => onSearch(e.target.value)}
          />
        </div>
      )}
      {action && <div className="ml-auto flex items-center gap-2">{action}</div>}
    </div>
  );
}
