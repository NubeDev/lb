// The MCP service page (tool-catalog scope) — the read-only detail page for the MCP runtime, drilled
// from the System page's MCP card. Shows the runtime summary (extension + tool counts) and the full
// catalog of reachable MCP tools — host-native + extension-contributed — as a searchable list grouped
// by source/family. READ-ONLY: it lists tools + descriptions, it never calls them. Obeys the UI
// standard (shadcn-first, `AppPageHeader`, responsive). Layout + wiring only; data lives in
// `useMcpService`.

import { useMemo, useState } from "react";
import { ArrowLeft, Plug, RefreshCw, Search } from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { ToolInfo } from "@/lib/system/system.types";
import { useMcpService } from "./useMcpService";

interface Props {
  ws: string;
  /** Back to the System page (the card this drilled from). Omitted in isolated tests. */
  onBack?: () => void;
}

export function McpServiceView({ ws, onBack }: Props) {
  const { mcpCard, tools, error, loading, refresh } = useMcpService();
  const [query, setQuery] = useState("");

  // Filter on the qualified name + description + source (case-insensitive substring).
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return tools;
    return tools.filter(
      (t) =>
        t.tool.toLowerCase().includes(q) ||
        t.description.toLowerCase().includes(q) ||
        t.source.toLowerCase().includes(q),
    );
  }, [tools, query]);

  // Group by `group` (host families + each ext id), groups sorted, tools already name-sorted upstream.
  const groups = useMemo(() => groupByGroup(filtered), [filtered]);

  return (
    <section className="flex h-full min-w-0 flex-col bg-bg text-fg">
      <AppPageHeader
        icon={Plug}
        title="MCP Service"
        description="The universal contract — every reachable MCP tool, with what it does."
        workspace={ws}
        actions={
          <div className="flex items-center gap-2">
            {mcpCard && (
              <>
                <CountBadge label="extensions" card={mcpCard} metric="extensions" />
                <CountBadge label="tools" card={mcpCard} metric="tools" />
              </>
            )}
            <Button
              variant="outline"
              size="sm"
              aria-label="refresh"
              disabled={loading}
              onClick={() => void refresh()}
            >
              <RefreshCw size={14} className={loading ? "animate-spin" : undefined} />
              Refresh
            </Button>
            {onBack && (
              <Button variant="ghost" size="sm" aria-label="back to system" onClick={onBack}>
                <ArrowLeft size={14} />
                System
              </Button>
            )}
          </div>
        }
      />

      {error && (
        <div
          role="alert"
          className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-sm text-destructive"
        >
          {error}
        </div>
      )}

      <div className="border-b border-border px-4 py-3">
        <div className="relative max-w-md">
          <Search
            size={14}
            className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"
            aria-hidden
          />
          <Input
            aria-label="search tools"
            placeholder="Search tools by name, description, or source…"
            className="pl-8"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <p className="mt-1.5 text-xs text-muted" aria-label="tool count">
          {filtered.length} of {tools.length} tool{tools.length === 1 ? "" : "s"}
          {query ? " match" : ""}
        </p>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-4">
        {tools.length === 0 ? (
          <p className="text-sm text-muted">{loading ? "Reading catalog…" : "No tools."}</p>
        ) : filtered.length === 0 ? (
          <p className="text-sm text-muted">No tools match “{query}”.</p>
        ) : (
          <div className="space-y-6">
            {groups.map(({ group, rows }) => (
              <ToolGroup key={group} group={group} rows={rows} />
            ))}
          </div>
        )}
      </div>
    </section>
  );
}

/** One group of tools (a host family like `host`/`agent`, or an extension id) with its rows. */
function ToolGroup({ group, rows }: { group: string; rows: ToolInfo[] }) {
  const isHost = rows[0]?.source === "host";
  return (
    <section aria-label={`group ${group}`}>
      <div className="mb-2 flex items-center gap-2">
        <h3 className="font-mono text-xs font-medium uppercase tracking-wide text-muted">{group}</h3>
        <Badge variant="outline" className="text-[10px]">
          {isHost ? "host" : "extension"}
        </Badge>
        <span className="text-xs text-muted">
          {rows.length} tool{rows.length === 1 ? "" : "s"}
        </span>
      </div>
      <ul className="divide-y divide-border rounded-md border border-border">
        {rows.map((t) => (
          <ToolRow key={t.tool} tool={t} />
        ))}
      </ul>
    </section>
  );
}

/** One tool: its qualified name (mono) + a one-line description. Extension tools may lack a description
 *  (the registry carries only names) — say so honestly rather than show a blank. */
function ToolRow({ tool }: { tool: ToolInfo }) {
  return (
    <li className="flex flex-col gap-0.5 px-3 py-2 sm:flex-row sm:items-baseline sm:gap-3">
      <code className="shrink-0 font-mono text-xs font-medium text-fg">{tool.tool}</code>
      <span className="text-xs text-muted">
        {tool.description || <span className="italic">no description provided</span>}
      </span>
    </li>
  );
}

function CountBadge({
  label,
  card,
  metric,
}: {
  label: string;
  card: { metrics: { label: string; value: string }[] };
  metric: string;
}) {
  const m = card.metrics.find((x) => x.label === metric);
  if (!m) return null;
  return (
    <Badge variant="outline" className="hidden gap-1.5 sm:inline-flex">
      <span className="text-muted">{label}</span>
      <span className="font-medium tabular-nums text-fg">{m.value}</span>
    </Badge>
  );
}

/** Group the (already name-sorted) tools by their `group`, with groups in stable alpha order but the
 *  caller's host families naturally sorting before/after ext ids by name. */
function groupByGroup(tools: ToolInfo[]): { group: string; rows: ToolInfo[] }[] {
  const map = new Map<string, ToolInfo[]>();
  for (const t of tools) {
    const arr = map.get(t.group) ?? [];
    arr.push(t);
    map.set(t.group, arr);
  }
  return [...map.entries()]
    .sort((a, b) => a[0].localeCompare(b[0]))
    .map(([group, rows]) => ({ group, rows }));
}
