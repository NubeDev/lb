// Effective caps WITH provenance (access-console scope) — renders a subject's resolved capability
// set, each cap tagged with WHERE it came from (direct / role:r / via team:t). Drives `authz.resolve`
// through `useResolveCaps`, so the displayed set and the enforced (token) set cannot drift. Honest
// loading/empty/denied states — never a fabricated cap, never a fake 0. One responsibility per file.

import { ShieldCheck } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useResolveCaps } from "./useResolveCaps";
import { SourceBadge } from "./SourceBadge";

interface Props {
  /** The `kind:name` subject, or null when nothing is selected. */
  subject: string | null;
}

export function EffectiveCaps({ subject }: Props) {
  const { caps, loading, error } = useResolveCaps(subject);

  if (!subject) return null;
  if (loading) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-1.5">
            <ShieldCheck size={14} /> Effective capabilities
          </CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-xs text-muted">Resolving…</p>
        </CardContent>
      </Card>
    );
  }
  if (error) {
    // A denied/failed resolve degrades to an honest note — never a fabricated cap set.
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-1.5">
            <ShieldCheck size={14} /> Effective capabilities
          </CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-xs text-destructive" role="alert">
            Could not resolve capabilities: {error}
          </p>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-1.5">
          <ShieldCheck size={14} /> Effective capabilities ({caps.length})
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-2">
        {caps.length === 0 ? (
          <p className="text-xs text-muted">
            No capabilities. Assign a role or a direct grant to give this subject access.
          </p>
        ) : (
          <ul className="space-y-1.5">
            {caps.map((c) => (
              <li key={c.cap} className="flex flex-col gap-1">
                <span className="break-all font-mono text-xs text-fg">{c.cap}</span>
                <span className="flex flex-wrap gap-1">
                  {c.source.map((s, i) => (
                    <SourceBadge key={i} source={s} />
                  ))}
                </span>
              </li>
            ))}
          </ul>
        )}
        <p className="pt-1 text-[0.6875rem] leading-relaxed text-muted">
          The union the session token carries (direct ∪ roles ∪ team-inherited). Cached caps drop on
          the subject&apos;s next sign-in; team membership changes are live.
        </p>
        {caps.length > 0 && (
          <Badge variant="outline" className="mt-1">
            client-side “who can do X” filters this set
          </Badge>
        )}
      </CardContent>
    </Card>
  );
}
