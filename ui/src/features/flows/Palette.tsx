// The node palette (flows-canvas scope, Wave 3). Renders the merged `flows.nodes` registry grouped
// by `category` (built-ins + every installed extension's `[[node]]` descriptors). Dragging a node
// type onto the canvas (or clicking it) adds a node instance — a `flow` record edit. A node whose
// underlying tool the caller lacks is SHOWN but marked gated (the menu reflects permissions; the
// deny lives at the engine, never widened — `caller ∩ grant`).
//
// flow-plain-wiring-scope: every port is plain per-message wiring (`any`) and is shown unmarked —
// a port is just a port. Only an EXPLICIT-`all` port (an extension descriptor opt-in; no built-in)
// wears the merge glyph, flagging the barrier exception. Styled as a compact, searchable catalog
// using shadcn primitives + tokens — the same look as the rest of the flows surface.

import { useMemo, useState } from "react";
import { Merge, Search } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import type { JoinPolicy, NodeDescriptor } from "@/lib/flows";
import { effectiveInputPorts } from "./flowGraph";
import { nodeIcon } from "./flowIcons";

interface PaletteProps {
  nodes: NodeDescriptor[];
  /** Add a node instance of this descriptor type to the canvas. */
  onAdd: (desc: NodeDescriptor) => void;
}

/** Group the registry by `category` (stable order: first-seen). Built-ins share the same grouping as
 *  extension nodes — one renderer, no `if native` branch. */
function byCategory(nodes: NodeDescriptor[]): { category: string; items: NodeDescriptor[] }[] {
  const order: string[] = [];
  const map = new Map<string, NodeDescriptor[]>();
  for (const n of nodes) {
    if (!map.has(n.category)) {
      map.set(n.category, []);
      order.push(n.category);
    }
    map.get(n.category)!.push(n);
  }
  return order.map((c) => ({ category: c, items: map.get(c)! }));
}

export function Palette({ nodes, onAdd }: PaletteProps) {
  const [query, setQuery] = useState("");
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return nodes;
    return nodes.filter(
      (n) =>
        n.title.toLowerCase().includes(q) ||
        n.type.toLowerCase().includes(q) ||
        n.category.toLowerCase().includes(q),
    );
  }, [nodes, query]);
  const groups = useMemo(() => byCategory(filtered), [filtered]);

  return (
    <div
      aria-label="flow palette"
      className="flex w-60 shrink-0 flex-col gap-2 overflow-y-auto border-r border-border bg-panel/50 p-2"
    >
      <div className="relative">
        <Search
          size={13}
          className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-muted"
        />
        <Input
          aria-label="search nodes"
          placeholder="Search nodes…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className="h-8 pl-7 text-xs"
        />
      </div>
      {groups.map((g) => (
        <div key={g.category} className="flex flex-col gap-1">
          <div className="px-1 text-[10px] font-semibold uppercase tracking-wide text-muted">
            {g.category}
          </div>
          {g.items.map((n) => {
            const Icon = nodeIcon(n);
            const ports = effectiveInputPorts(n);
            return (
              <Button
                key={n.type}
                type="button"
                variant="outline"
                aria-label={`add node ${n.type}`}
                draggable
                onDragStart={(e) => e.dataTransfer.setData("application/x-flow-node", n.type)}
                onClick={() => onAdd(n)}
                className={cn(
                  "flex h-auto items-center gap-2 rounded-md border-border bg-card px-2 py-1.5 text-left text-xs shadow-none hover:border-accent/40 hover:bg-accent/5",
                )}
              >
                <Icon size={15} className="shrink-0 text-accent" />
                <span className="min-w-0 flex-1">
                  <span className="block truncate font-medium text-fg">{n.title}</span>
                  <span className="block truncate text-[10px] text-muted">{n.type}</span>
                  {ports.length > 0 ? (
                    <span className="mt-0.5 flex flex-wrap items-center gap-x-1.5 gap-y-0.5 text-[10px] text-muted">
                      {ports.map((p) => (
                        <span
                          key={p.name}
                          className="inline-flex items-center gap-0.5"
                          title={
                            p.join === "all"
                              ? `${p.name}: all (join — barrier over this port's upstreams)`
                              : `${p.name}: fires once per arriving message`
                          }
                        >
                          <PolicyMark join={p.join} />
                          {p.name}
                        </span>
                      ))}
                    </span>
                  ) : null}
                </span>
              </Button>
            );
          })}
        </div>
      ))}
      {nodes.length === 0 ? (
        <div className="px-1 text-xs text-muted">No nodes.</div>
      ) : filtered.length === 0 ? (
        <div className="px-1 text-xs text-muted">No nodes match “{query}”.</div>
      ) : null}
    </div>
  );
}

/** The compact glyph marking ONLY an explicit-`all` (barrier) port in the palette — the default
 *  (`any`, plain per-message wiring) is unmarked (flow-plain-wiring-scope). Mirrors the canvas
 *  node's per-port glyph. */
function PolicyMark({ join }: { join: JoinPolicy }) {
  if (join !== "all") return null;
  return <Merge size={9} className="text-muted" aria-hidden />;
}
