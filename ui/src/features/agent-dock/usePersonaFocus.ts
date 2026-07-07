// The dock persona FOCUS resolver (persona-session #5) — the client-side half of the five-layer
// precedence (scope's resolved model): PIN > CONTEXT MATCH > (server: member default > ws default > none).
// The hook fetches the persona roster ONCE (`agent.persona.list`, tolerating a deny ⇒ no suggestion),
// reads the LIVE page surface (the dock already captures it per-message), and resolves the focus the
// chip should display AND the dock should send as the per-invoke `persona` arg.
//
// One responsibility: derive { current, options, pin, clearPin } from roster + surface + pin. The
// pin storage lives in `personaPin.ts`; the chip presentation lives in `DockPersonaChip.tsx`.
// Rule 10 (no core branch on ids): the surface match is pure DATA over the roster the host already
// returned — the host never sees a surface→persona rule, only the resolved per-invoke `persona` arg.

import { useCallback, useEffect, useMemo, useState } from "react";

import { listPersonas, type PersonaListItem } from "@/lib/agent/agentPersona.api";
import { readPersonaPin, writePersonaPin, clearPersonaPin } from "./personaPin";

/** Why a persona is the current focus — drives the chip's caption. */
export type PersonaFocusReason = "pinned" | "context";

/** The resolved focus the dock chip displays and the dock sends as the per-invoke `persona` arg. */
export interface PersonaFocus {
  /** The current persona the dock will send (and why), or `null` to send NO persona and let the
   *  server fold member→ws-default prefs (which may land on none ⇒ un-narrowed run). */
  current: { id: string; label: string; reason: PersonaFocusReason } | null;
  /** The context-suggested persona (the first enabled roster entry whose `surfaces` includes the live
   *  surface, in roster order). `null` when nothing matches or no roster loaded. */
  suggestion: PersonaListItem | null;
  /** Enabled personas (the switcher options). Disabled ones are hidden from the picker + match. */
  options: PersonaListItem[];
  /** The raw roster (the chip header may show a count / "no personas available" when this is empty). */
  roster: PersonaListItem[];
  /** The currently-pinned id (`null` when no pin in this tab). */
  pinId: string | null;
  /** Pin `id` for THIS tab (overrides the context match). Sticks in `sessionStorage` until cleared. */
  pin: (id: string) => void;
  /** Clear the pin → return to the context match (or none). Per-tab; never affects another member/tab. */
  clearPin: () => void;
  /** `true` until the first roster fetch settles (the chip waits to avoid a flash). */
  loading: boolean;
}

/**
 * Resolve the dock's persona focus for `ws` against the LIVE `surface`. The caller passes the live
 * surface (NOT the whole page context) — the dock already captures it per-message; here we only need
 * the surface string for the match. Tolerates a denied `agent.persona.list` (folds to empty + no
 * suggestion — the run lands on the server's prefs fold, never an errored run).
 */
export function usePersonaFocus(ws: string, surface: string): PersonaFocus {
  const [roster, setRoster] = useState<PersonaListItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [pinId, setPinId] = useState<string | null>(() => readPersonaPin(ws));

  // Fetch the roster once per workspace. The roster changes rarely (an admin write); the chip's pin
  // and surface changes drive the live re-resolution below. A denied list ⇒ empty (no suggestion).
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    void listPersonas()
      .then((rows) => {
        if (!cancelled) setRoster(rows);
      })
      .catch(() => {
        /* denied / offline — fold to no-suggestion (the server still resolves a default at invoke) */
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [ws]);

  // If the workspace changes, re-read the pin from the new workspace's sessionStorage key.
  useEffect(() => {
    setPinId(readPersonaPin(ws));
  }, [ws]);

  const pin = useCallback(
    (id: string) => {
      writePersonaPin(ws, id);
      setPinId(readPersonaPin(ws));
    },
    [ws],
  );
  const clearPin = useCallback(() => {
    clearPersonaPin(ws);
    setPinId(readPersonaPin(ws));
  }, [ws]);

  // The enabled roster (None = all enabled is computed server-side and carried as `enabled` on each row;
  // here we just filter). Disabled personas are hidden from BOTH the picker and the context match.
  const options = useMemo(() => roster.filter((p) => p.enabled), [roster]);

  // The context suggestion: first ENABLED persona whose `surfaces` includes the live surface, in roster
  // (id-sorted) order. The scope resolved open-q #2 as id-sorted (the host's list order), not seed order.
  const suggestion = useMemo(
    () => options.find((p) => p.surfaces.includes(surface)) ?? null,
    [options, surface],
  );

  // Resolve the focus: pin > context match > null (null ⇒ server folds prefs). The chip's "why" caption
  // mirrors this exactly so the chip and the per-invoke `persona` arg never disagree.
  const current = useMemo<{ id: string; label: string; reason: PersonaFocusReason } | null>(() => {
    if (pinId) {
      const pinned = roster.find((p) => p.id === pinId);
      // A pinned id that left the roster (deleted) or got disabled: silently fall through to the
      // suggestion rather than holding a stale pin (the host would 400 on an explicit disabled id).
      if (pinned && pinned.enabled) {
        return { id: pinned.id, label: pinned.label, reason: "pinned" };
      }
    }
    if (suggestion) {
      return { id: suggestion.id, label: suggestion.label, reason: "context" };
    }
    return null;
  }, [pinId, roster, suggestion]);

  return { current, suggestion, options, roster, pinId, pin, clearPin, loading };
}
