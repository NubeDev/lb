// The shadcn-style `select` primitive, repointed at the Lazybones tokens (`bg`/`fg`/`border`/
// `muted`) the way `sidebar.tsx` binds the upstream component to our palette (ui-standards-scope,
// component backlog). A styled wrapper over the native `<select>` — token-bound, accessible,
// mobile-friendly (the native picker), no Radix dependency. One primitive per file (FILE-LAYOUT).

import * as React from "react";

import { cn } from "@/lib/utils";

function Select({ className, ...props }: React.ComponentProps<"select">) {
  return (
    <select
      data-slot="select"
      className={cn(
        "h-9 w-full rounded-md border border-border bg-bg px-3 text-xs text-fg transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/30 disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      {...props}
    />
  );
}

export { Select };
