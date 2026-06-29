// The Chains page (rules-workbench scope, Phase 2) — wraps the rail (list/open/delete) + the DAG
// canvas (edit/save/run/settle). The page is thin glue: it owns the open chain via `useChains` and
// hands the canvas a save action that surfaces the host's validation message inline.

import { useCallback, useState } from "react";

import type { Chain } from "@/lib/chains";
import { ChainCanvas } from "./ChainCanvas";
import { ChainRail } from "./ChainRail";
import { useChains } from "./useChains";

export interface ChainsViewProps {
  ws: string;
}

/** A fresh blank chain (a single starter step the author renames/rewires). */
function blankChain(): Chain {
  const id = `chain-${Date.now()}`;
  return { id, name: id, steps: [] };
}

export function ChainsView({ ws }: ChainsViewProps) {
  const { roster, open, error, load, save, remove, setOpen } = useChains(ws);
  const [draftId, setDraftId] = useState(0); // bump to force a fresh canvas on "new"

  const onNew = useCallback(() => {
    setOpen(blankChain());
    setDraftId((n) => n + 1);
  }, [setOpen]);

  return (
    <div aria-label="chains view" style={{ display: "flex", height: "100%" }}>
      <ChainRail
        roster={roster}
        openId={open?.id ?? null}
        onOpen={load}
        onDelete={remove}
        onNew={onNew}
      />
      {error ? (
        <div aria-label="chains error" style={{ color: "#dc2626", padding: 8 }}>
          {error}
        </div>
      ) : null}
      {open ? (
        <ChainCanvas key={`${open.id}-${draftId}`} chain={open} onSave={save} />
      ) : (
        <div style={{ padding: 16, color: "#9ca3af" }}>Select or create a chain.</div>
      )}
    </div>
  );
}
