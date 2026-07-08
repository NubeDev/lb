// useAutoFormat — the reactive read/write of the rules editor auto-format preference
// (rules-editor-ux scope). One responsibility: expose the persisted flag as React state and a
// setter that writes through to `localStorage`, so a toggle in the header and the editor's
// auto-format-on-blur read the same source of truth. The initial value is read lazily (once) so the
// first render already reflects the persisted choice — no flash of the wrong toggle state.
//
// FILE-LAYOUT: one hook per file. The storage primitives live in `autoFormatPref.ts`.

import { useCallback, useState } from "react";

import { readAutoFormat, writeAutoFormat } from "./autoFormatPref";

export interface AutoFormatState {
  /** Whether auto-format is enabled (persisted in localStorage, browser-wide). */
  enabled: boolean;
  /** Flip the flag and persist it. */
  toggle: () => void;
}

export function useAutoFormat(): AutoFormatState {
  const [enabled, setEnabled] = useState<boolean>(() => readAutoFormat());

  const toggle = useCallback(() => {
    setEnabled((prev) => {
      const next = !prev;
      writeAutoFormat(next);
      return next;
    });
  }, []);

  return { enabled, toggle };
}
