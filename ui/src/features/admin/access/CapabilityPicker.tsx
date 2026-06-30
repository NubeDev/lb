// The guided capability picker (access-console scope) — a catalog-driven grant entry that replaces
// the raw-string-first field. It reads `tools.catalog` (already caller-cap-filtered, so the offered
// caps are ⊆ the admin's own resolved caps — NO widening: the UI cannot offer a grant the gateway
// would reject), groups tools by their extension/surface, and on pick emits the CANONICAL cap string
// `mcp:<tool>:call` for `grants.assign`. The raw-string escape hatch stays (in AccessEditor) for
// power-users / non-`mcp:` surfaces. One responsibility per file (FILE-LAYOUT).

import { useMemo, useState } from "react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Select } from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { useCatalog } from "@/features/channel/palette/useCatalog";

interface Props {
  /** Called with the canonical cap string (`mcp:<tool>:call`) when the admin picks + confirms. */
  onPick: (cap: string) => void;
  /** Caps already granted directly, to mark as granted (no duplicate grant needed). */
  alreadyGranted?: string[];
}

/** Canonical cap string for an MCP tool name: `mcp:<tool>:call`. */
export function toolToCap(tool: string): string {
  return `mcp:${tool}:call`;
}

export function CapabilityPicker({ onPick, alreadyGranted = [] }: Props) {
  const { tools, loading, error } = useCatalog();
  const [selected, setSelected] = useState("");

  // Group tools by their surface/extension, sorted for stable display.
  const grouped = useMemo(() => {
    const byGroup = new Map<string, string[]>();
    for (const t of tools) (byGroup.get(t.group) ?? byGroup.set(t.group, []).get(t.group)!).push(t.name);
    for (const names of byGroup.values()) names.sort();
    return [...byGroup.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  }, [tools]);

  function commit() {
    if (!selected) return;
    onPick(toolToCap(selected));
    setSelected("");
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Grant a capability</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2">
        <p className="text-[0.6875rem] leading-relaxed text-muted">
          Picked from the tools you are authorized for, so you can only grant caps you hold
          (no-widening). Need a raw <code className="font-mono">store:</code>/<code className="font-mono">bus:</code> string or a power-user cap? Use the Advanced drawer.
        </p>
        {loading ? (
          <p className="text-xs text-muted">Loading catalog…</p>
        ) : error ? (
          <p className="text-xs text-destructive" role="alert">
            Catalog unavailable: {error}
          </p>
        ) : tools.length === 0 ? (
          <p className="text-xs text-muted">
            No tools authorized. You hold no grantable MCP capability.
          </p>
        ) : (
          <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
            <Select
              aria-label="pick a capability to grant"
              className="sm:max-w-sm"
              value={selected}
              onChange={(e) => setSelected(e.target.value)}
            >
              <option value="">Pick a tool…</option>
              {grouped.map(([group, names]) => (
                <optgroup key={group} label={group}>
                  {names.map((n) => {
                    const cap = toolToCap(n);
                    return (
                      <option key={n} value={n}>
                        {n} ({cap}){alreadyGranted.includes(cap) ? " — already granted" : ""}
                      </option>
                    );
                  })}
                </optgroup>
              ))}
            </Select>
            <Button type="button" size="sm" disabled={!selected} onClick={commit}>
              Grant
            </Button>
            {selected && (
              <Badge variant="outline" className="font-mono">
                {toolToCap(selected)}
              </Badge>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
