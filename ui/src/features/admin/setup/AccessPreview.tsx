// The live access preview (setup scope) — the wizard's final step renders, for the person being
// onboarded, an HONEST mock of the sidebar they'll see once they log in. It mirrors the TWO real
// rendering paths in `NavRail` (see `previewReach.ts`):
//
//   • A nav applies (the wizard shared one) → render ONLY that nav's items, cap-stripped. This is why
//     a 1-item nav previews as 1 item — an applied nav REPLACES the surface list.
//   • No nav → render the built-in fallback (`allowedSurfaces(caps)`).
//
// The caps are resolved server-side (`authz.resolve`) so the preview is honest; the strip reuses the
// shell's own gate so preview and rail can't drift. A lens, never a grant — the gateway re-checks every
// verb regardless (rule 5). One responsibility per file (FILE-LAYOUT).

import { useEffect, useState } from "react";
import { Eye, Loader2 } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { SURFACE_DEF } from "@/features/shell/surfaceDefs";
import type { SourcedCap } from "@/lib/admin/grants.api";
import type { NavItem } from "@/lib/nav";
import {
  capWhy,
  flattenCaps,
  previewFallbackRows,
  previewNavRows,
  type PreviewRow,
} from "./previewReach";

interface Props {
  /** The bare user id being previewed. */
  user: string;
  /** Resolve the user's effective sourced-caps (the honest input). Reruns when `user`/`nonce` change. */
  resolve: (user: string) => Promise<SourcedCap[]>;
  /** Bump to force a re-resolve after a write in an earlier step (e.g. a role just granted). */
  nonce: number;
  /** The nav that will apply for this user (its items), or `null` when the built-in sidebar is kept.
   *  Drives whether the preview shows the nav's stripped items or the fallback surface set. */
  navItems: NavItem[] | null;
  /** The applied nav's title (for the header line) — optional. */
  navTitle?: string;
}

export function AccessPreview({ user, resolve, nonce, navItems, navTitle }: Props) {
  const [rows, setRows] = useState<PreviewRow[]>([]);
  const [sourced, setSourced] = useState<SourcedCap[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    setLoading(true);
    setError(null);
    resolve(user)
      .then((caps) => {
        if (!live) return;
        setSourced(caps);
        const flat = flattenCaps(caps);
        // A nav applies → its stripped items; else the built-in fallback (exactly like NavRail).
        setRows(navItems && navItems.length ? previewNavRows(navItems, flat) : previewFallbackRows(flat));
      })
      .catch((e) => live && setError(e instanceof Error ? e.message : String(e)))
      .finally(() => live && setLoading(false));
    return () => {
      live = false;
    };
  }, [user, resolve, nonce, navItems]);

  // Roll the provenance up to a per-source tally — the honest "why".
  const roles = new Set<string>();
  const teams = new Set<string>();
  let direct = 0;
  for (const c of sourced) {
    for (const w of capWhy(c)) {
      if (w === "direct") direct += 1;
      else if ("role" in w) roles.add(w.role);
      else teams.add(w.team);
    }
  }

  const applied = !!(navItems && navItems.length);

  return (
    <div className="grid gap-4 md:grid-cols-[minmax(0,15rem)_1fr]">
      {/* ── The faux sidebar — what the person actually sees ── */}
      <div className="overflow-hidden rounded-xl border border-border bg-bg shadow-sm">
        <div className="flex items-center gap-2 border-b border-border bg-panel px-3 py-2.5">
          <Eye size={14} className="text-primary" />
          <span className="truncate text-xs font-semibold text-fg">{user}&rsquo;s sidebar</span>
        </div>
        <div className="p-2">
          {loading ? (
            <div className="flex items-center gap-2 px-2 py-4 text-xs text-muted">
              <Loader2 size={14} className="animate-spin" /> Resolving access…
            </div>
          ) : error ? (
            <div className="px-2 py-4 text-xs text-destructive" role="alert">
              {error}
            </div>
          ) : rows.length === 0 ? (
            <div className="px-2 py-4 text-xs text-muted">
              {applied
                ? "This menu has no pages this user can reach."
                : "No pages — this user sees an empty rail."}
            </div>
          ) : (
            <ul className="space-y-0.5" data-testid="preview-rail">
              {rows.map((r, i) => {
                const Icon = SURFACE_DEF[r.icon]?.icon;
                return (
                  <li key={`${r.label}-${i}`}>
                    <span className="flex items-center gap-2.5 rounded-md px-2 py-1.5 text-sm text-fg hover:bg-panel">
                      {Icon ? <Icon size={15} className="text-muted" /> : null}
                      {r.label}
                    </span>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </div>

      {/* ── The summary — WHAT and WHY ── */}
      <div className="space-y-3">
        <div>
          <h4 className="text-xs font-semibold uppercase tracking-wide text-muted">What they&rsquo;ll see</h4>
          <p className="mt-1 text-sm text-fg">
            {applied ? (
              <>
                Their sidebar is the{" "}
                <span className="font-medium">{navTitle || "custom"}</span> menu —{" "}
              </>
            ) : (
              <>They see the built-in sidebar — </>
            )}
            <span className="font-medium">{rows.length}</span> page{rows.length === 1 ? "" : "s"}
            {applied ? " (the rest of the workspace is hidden by this menu)" : ""}.
          </p>
        </div>

        <div>
          <h4 className="text-xs font-semibold uppercase tracking-wide text-muted">
            Where the access comes from
          </h4>
          <div className="mt-1.5 flex flex-wrap gap-1.5">
            {roles.size === 0 && teams.size === 0 && direct === 0 ? (
              <span className="text-sm text-muted">
                No capabilities yet — assign a role in the Access step.
              </span>
            ) : (
              <>
                {[...roles].map((r) => (
                  <Badge key={`role:${r}`} variant="secondary">
                    role: {r}
                  </Badge>
                ))}
                {[...teams].map((t) => (
                  <Badge key={`team:${t}`} variant="outline">
                    via team: {t}
                  </Badge>
                ))}
                {direct > 0 && <Badge variant="outline">{direct} direct</Badge>}
              </>
            )}
          </div>
        </div>

        <p className="text-xs text-muted">
          {applied
            ? "The menu only HIDES pages — it grants nothing. The gateway still re-checks every action server-side."
            : "This is a live preview of the display gate — the gateway still re-checks every action server-side."}
        </p>
      </div>
    </div>
  );
}
