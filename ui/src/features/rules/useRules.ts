// The rules workbench hook — the one place the Playground loads the saved-rule roster, the editor
// buffer, runs the current buffer, and CRUDs a rule (rules-workbench scope, Phase 1). Every call goes
// through the real `invoke` seam to the gateway/host; no fake/demo data (CLAUDE §9). The buffer is
// transient component state (an unsaved edit) — the saved record is the durable truth (rule 4). A run
// uses the BUFFER (ad-hoc body), never silently the saved version when they differ. One hook per file.

import { useCallback, useEffect, useState } from "react";

import {
  deleteRule,
  getRule,
  listRules,
  runRule,
  saveRule,
  type RunResult,
  type SavedRule,
} from "@/lib/rules";

export interface RulesState {
  roster: SavedRule[];
  /** The id of the currently-open saved rule, or null for a fresh ad-hoc buffer. */
  selectedId: string | null;
  /** The editor buffer (the Rhai body being authored — may diverge from the saved record). */
  buffer: string;
  /** The buffer's saved body, for the dirty check (null when nothing is open). */
  savedBody: string | null;
  result: RunResult | null;
  /** The last error message — a generic deny ("not permitted") or verbatim author feedback. */
  error: string | null;
  running: boolean;
  dirty: boolean;
  setBuffer: (body: string) => void;
  refresh: () => Promise<void>;
  open: (id: string) => Promise<void>;
  newRule: () => void;
  /** Load an example body into the buffer; if there are unsaved edits, confirm before clobbering. */
  loadExample: (body: string) => void;
  run: () => Promise<void>;
  save: (id: string, name: string) => Promise<void>;
  remove: (id: string) => Promise<void>;
}

/** Coerce any thrown value to a message (an `invoke` rejection carries the gateway body — generic for a
 *  403, verbatim author feedback for a 400). */
function msg(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

export function useRules(_ws: string): RulesState {
  const [roster, setRoster] = useState<SavedRule[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [buffer, setBuffer] = useState("");
  const [savedBody, setSavedBody] = useState<string | null>(null);
  const [result, setResult] = useState<RunResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setRoster(await listRules());
    } catch (e) {
      setError(msg(e));
    }
  }, []);

  // The workspace is derived from the token server-side; a change of `_ws` (sign-in / switch) reloads.
  useEffect(() => {
    void refresh();
  }, [refresh, _ws]);

  const open = useCallback(async (id: string) => {
    setError(null);
    setResult(null);
    try {
      const rule = await getRule(id);
      setSelectedId(rule.id);
      setBuffer(rule.body);
      setSavedBody(rule.body);
    } catch (e) {
      setError(msg(e));
    }
  }, []);

  const newRule = useCallback(() => {
    setSelectedId(null);
    setBuffer("");
    setSavedBody(null);
    setResult(null);
    setError(null);
  }, []);

  const dirty = savedBody !== null && buffer !== savedBody;

  const loadExample = useCallback(
    (body: string) => {
      // Respect the dirty indicator: a fresh buffer with unsaved edits isn't silently clobbered.
      const hasUnsaved = buffer.trim() !== "" && (savedBody === null || buffer !== savedBody);
      if (hasUnsaved && typeof window !== "undefined") {
        const ok = window.confirm("Replace the editor with this example? Unsaved edits will be lost.");
        if (!ok) return;
      }
      // An example is an ad-hoc body — detach from any open saved rule.
      setSelectedId(null);
      setSavedBody(null);
      setBuffer(body);
      setResult(null);
      setError(null);
    },
    [buffer, savedBody],
  );

  const run = useCallback(async () => {
    setRunning(true);
    setError(null);
    try {
      // The run uses the editor BUFFER (ad-hoc body) — what the author sees is what runs.
      setResult(await runRule({ body: buffer }));
    } catch (e) {
      setResult(null);
      setError(msg(e));
    } finally {
      setRunning(false);
    }
  }, [buffer]);

  const save = useCallback(
    async (id: string, name: string) => {
      setError(null);
      try {
        await saveRule({ id, name, body: buffer });
        setSelectedId(id);
        setSavedBody(buffer);
        await refresh();
      } catch (e) {
        setError(msg(e));
      }
    },
    [buffer, refresh],
  );

  const remove = useCallback(
    async (id: string) => {
      setError(null);
      try {
        await deleteRule(id);
        if (selectedId === id) newRule();
        await refresh();
      } catch (e) {
        setError(msg(e));
      }
    },
    [selectedId, newRule, refresh],
  );

  return {
    roster,
    selectedId,
    buffer,
    savedBody,
    result,
    error,
    running,
    dirty,
    setBuffer,
    refresh,
    open,
    newRule,
    loadExample,
    run,
    save,
    remove,
  };
}
