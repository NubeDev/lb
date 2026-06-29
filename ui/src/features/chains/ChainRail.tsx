// The chain rail (rules-workbench scope, Phase 2) — the left list of saved chains: open one, delete
// one, or start a new blank chain. Presentation only; the roster + actions come from the parent's
// `useChains` hook.

import { Button } from "@/components/ui/button";
import type { ChainSummary } from "@/lib/chains";

export interface ChainRailProps {
  roster: ChainSummary[];
  openId: string | null;
  onOpen: (id: string) => void;
  onDelete: (id: string) => void;
  onNew: () => void;
}

export function ChainRail({ roster, openId, onOpen, onDelete, onNew }: ChainRailProps) {
  return (
    <div aria-label="chain rail" style={{ width: 200, borderRight: "1px solid #e5e7eb", padding: 8 }}>
      <Button
        aria-label="new chain"
        onClick={onNew}
        variant="outline"
        size="sm"
        style={{ width: "100%", marginBottom: 8 }}
      >
        + New chain
      </Button>
      {roster.length === 0 ? (
        <div style={{ color: "#9ca3af", fontSize: 12 }}>No chains yet.</div>
      ) : (
        <ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
          {roster.map((c) => (
            <li key={c.id} style={{ display: "flex", alignItems: "center", gap: 4 }}>
              <Button
                aria-label={`open chain ${c.id}`}
                onClick={() => onOpen(c.id)}
                variant="ghost"
                size="sm"
                style={{ flex: 1, justifyContent: "flex-start", fontWeight: c.id === openId ? 700 : 400 }}
              >
                {c.name || c.id}
              </Button>
              <Button
                aria-label={`delete chain ${c.id}`}
                onClick={() => onDelete(c.id)}
                variant="ghost"
                size="sm"
              >
                ✕
              </Button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
