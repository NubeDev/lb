// A provenance badge — renders ONE `CapSource` (direct / role:r / via team:t) as a labeled Badge so
// an admin sees *why* a subject holds each cap (access-console scope). One responsibility per file.

import { Badge } from "@/components/ui/badge";
import type { CapSource } from "@/lib/admin/grants.api";

/** A short human label for a source (used by the badge + the overview). */
export function sourceLabel(s: CapSource): string {
  switch (s.kind) {
    case "direct":
      return "direct";
    case "role":
      return `role: ${s.name}`;
    case "team":
      return `via team: ${s.name}`;
  }
}

export function SourceBadge({ source }: { source: CapSource }) {
  return (
    <Badge
      variant={source.kind === "direct" ? "secondary" : "outline"}
      className="font-normal"
    >
      {sourceLabel(source)}
    </Badge>
  );
}
