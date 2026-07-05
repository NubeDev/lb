// The Effective-tools read-only view (agent-personas scope #1) — the load-bearing boundary made
// visible. It resolves a persona (`agent.persona.resolve`) and reads the caller's REAL reachable tools
// (`tools.catalog`), then shows the live `persona ∩ agent ∩ caller` result: each catalog tool the
// persona advertises as included, and each `granted_tools` entry NOT in the catalog as EXCLUDED with a
// reason ("not granted"). It also shows the resolved identity + pinned skills. There is NO grant
// editing here — the wall is untouched; a note links out to Roles & Grants for the caps themselves.
//
// Tool ids / globs are OPAQUE (rule 10): matching is a trailing-`*` prefix test and nothing smarter.

import { useEffect, useState } from "react";

import { resolveEffectivePersona, type EffectivePersona } from "@/lib/agent/agentPersona.api";
import { toolsCatalog } from "@/lib/channel/channel.api";

interface Props {
  /** The persona to resolve — the active or the selected id. Absent → the workspace's active persona. */
  personaId?: string;
}

/** Does a persona `granted_tools` glob match a concrete tool name? Trailing-`*` = prefix match on the
 *  tool id; otherwise an exact match. Deliberately dumb (scope: "prefix-on-the-tool-id and nothing
 *  smarter") — the ids are opaque, so no cap-grammar interplay. */
function matches(glob: string, tool: string): boolean {
  if (glob.endsWith("*")) return tool.startsWith(glob.slice(0, -1));
  return glob === tool;
}

type Row = { tool: string; status: "included" | "excluded"; reason?: string };

export function EffectiveTools({ personaId }: Props) {
  const [effective, setEffective] = useState<EffectivePersona | null>(null);
  const [catalog, setCatalog] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    setLoading(true);
    setError(null);
    void Promise.allSettled([resolveEffectivePersona(personaId), toolsCatalog()]).then(
      ([eff, cat]) => {
        if (!live) return;
        setEffective(eff.status === "fulfilled" ? eff.value : null);
        setCatalog(cat.status === "fulfilled" ? cat.value.tools.map((t) => t.name) : []);
        if (eff.status === "rejected")
          setError(eff.reason instanceof Error ? eff.reason.message : "resolve failed");
        setLoading(false);
      },
    );
    return () => {
      live = false;
    };
  }, [personaId]);

  if (loading) return <p className="text-sm text-muted">Loading…</p>;
  if (error)
    return (
      <p role="alert" className="text-xs text-red-500">
        {error}
      </p>
    );
  if (!effective)
    return (
      <p className="rounded-md border border-dashed border-border px-4 py-6 text-center text-sm text-muted">
        No persona resolves for this workspace — the agent runs with the full reachable menu.
      </p>
    );

  const catalogSet = new Set(catalog);

  // For each persona granted_tools entry: a concrete tool in the caller's catalog is INCLUDED; a
  // concrete tool absent from the catalog is EXCLUDED ("not granted" — the wall would deny it); a glob
  // includes every catalog tool it prefixes (and nothing when it prefixes none). This is exactly the
  // advertised menu ∩ the caller's real grants — the live intersection, per-exclusion labelled.
  const rows: Row[] = [];
  const seen = new Set<string>();
  for (const glob of effective.granted_tools) {
    if (glob.endsWith("*")) {
      const hits = catalog.filter((t) => matches(glob, t));
      if (hits.length === 0) {
        rows.push({ tool: glob, status: "excluded", reason: "no granted tool matches this glob" });
      } else {
        for (const t of hits) {
          if (!seen.has(t)) {
            seen.add(t);
            rows.push({ tool: t, status: "included" });
          }
        }
      }
    } else {
      if (seen.has(glob)) continue;
      seen.add(glob);
      rows.push(
        catalogSet.has(glob)
          ? { tool: glob, status: "included" }
          : { tool: glob, status: "excluded", reason: "not granted" },
      );
    }
  }

  const includedCount = rows.filter((r) => r.status === "included").length;

  return (
    <div className="flex flex-col gap-3" aria-label="effective tools">
      <div className="rounded-md border border-border px-4 py-3">
        <p className="text-[11px] uppercase tracking-wide text-muted">Identity</p>
        <p className="mt-1 whitespace-pre-wrap text-sm text-fg">
          {effective.identity || <span className="text-muted">— none —</span>}
        </p>
      </div>

      {effective.grounding_skills.length > 0 && (
        <div className="rounded-md border border-border px-4 py-3">
          <p className="text-[11px] uppercase tracking-wide text-muted">Pinned skills</p>
          <ul className="mt-1 flex flex-wrap gap-1.5" aria-label="pinned skills">
            {effective.grounding_skills.map((s) => (
              <li key={s} className="rounded-md bg-panel px-2 py-0.5 font-mono text-xs text-fg">
                {s}
              </li>
            ))}
          </ul>
        </div>
      )}

      <div>
        <p className="mb-1 text-[11px] text-muted">
          {includedCount} tool{includedCount === 1 ? "" : "s"} reachable ·{" "}
          {rows.length - includedCount} excluded — the live{" "}
          <span className="font-medium">persona ∩ agent ∩ caller</span> for you.
        </p>
        {rows.length === 0 ? (
          <p className="rounded-md border border-dashed border-border px-4 py-4 text-center text-sm text-muted">
            This persona narrows nothing — the agent sees your full reachable menu.
          </p>
        ) : (
          <ul className="flex flex-col gap-1" aria-label="effective tool rows">
            {rows.map((r) => (
              <li
                key={r.tool}
                aria-label={`tool ${r.tool}`}
                data-status={r.status}
                className="flex items-center justify-between gap-3 rounded-md border border-border px-3 py-1.5"
              >
                <span className="truncate font-mono text-xs text-fg">{r.tool}</span>
                {r.status === "included" ? (
                  <span className="shrink-0 rounded-md bg-accent/15 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-accent">
                    Included
                  </span>
                ) : (
                  <span className="shrink-0 text-[11px] text-amber-500" role="note">
                    Excluded · {r.reason}
                  </span>
                )}
              </li>
            ))}
          </ul>
        )}
      </div>

      <p className="border-t border-border pt-3 text-[11px] text-muted">
        This is a read-only view of advertisement, not the wall. To grant or revoke a capability, use{" "}
        <span className="font-medium text-fg">Roles &amp; Grants</span> — a persona never widens what
        you can reach.
      </p>
    </div>
  );
}
