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
  type RuleParam,
  type RunResult,
  type SavedRule,
} from "@/lib/rules";

export interface RulesState {
  roster: SavedRule[];
  /** The id of the currently-open saved rule, or null for a fresh ad-hoc buffer. */
  selectedId: string | null;
  /** The name of the currently-open rule (tracked locally so the header is correct even while
   *  `rules.list` is stale); null when nothing is open. */
  name: string | null;
  /** The editor buffer (the Rhai body being authored — may diverge from the saved record). */
  buffer: string;
  /** The declared params being authored (co-owned with the body; persisted by save/create/rename). */
  params: RuleParam[];
  /** The buffer's saved body, for the dirty check (null when nothing is open). */
  savedBody: string | null;
  result: RunResult | null;
  /** The last error message — a generic deny ("not permitted") or verbatim author feedback. */
  error: string | null;
  running: boolean;
  dirty: boolean;
  setBuffer: (body: string) => void;
  /** Replace the declared params (the params editor's onChange). */
  setParams: (params: RuleParam[]) => void;
  refresh: () => Promise<void>;
  open: (id: string) => Promise<void>;
  newRule: () => void;
  /** Create + save a new named rule from the current buffer. Derives a unique slug id from `name`,
   *  persists it, and opens it. Returns the new id (or null on failure). */
  create: (name: string) => Promise<string | null>;
  /** Load an example body into the buffer; if there are unsaved edits, confirm before clobbering. */
  loadExample: (body: string) => void;
  run: () => Promise<void>;
  save: (id: string, name: string) => Promise<void>;
  /** The single "persist what I'm looking at" action the toolbar/⌘S calls. If a saved rule is open it
   *  updates it in place; if this is a fresh ad-hoc buffer it needs a name — returns
   *  `{ needsName: true }` so the caller can open the inline name field instead of silently failing. */
  saveCurrent: (nameForNew?: string) => Promise<{ ok: boolean; needsName: boolean }>;
  /** True once a run has completed (success or typed error) — lets the result region distinguish
   *  "you haven't run yet" from "ran and returned nothing". */
  hasRun: boolean;
  /** Rename the currently-open rule (same id, new name). Preserves the persisted body. */
  rename: (name: string) => Promise<boolean>;
  remove: (id: string) => Promise<void>;
}

/** Coerce any thrown value to a message (an `invoke` rejection carries the gateway body — generic for a
 *  403, verbatim author feedback for a 400). */
function msg(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** Derive a stable, URL-safe id from a human `name`: lowercase, non-alphanumerics → `-`, trimmed.
 *  The id is the rule's store key (an implementation detail); the user only ever types the name. */
function slugify(name: string): string {
  const slug = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return slug || "rule";
}

/** Append `-2`, `-3`, … to `base` until it doesn't collide with an existing id in `taken`. */
function uniqueId(base: string, taken: Set<string>): string {
  if (!taken.has(base)) return base;
  let n = 2;
  while (taken.has(`${base}-${n}`)) n += 1;
  return `${base}-${n}`;
}

export function useRules(_ws: string): RulesState {
  const [roster, setRoster] = useState<SavedRule[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [name, setName] = useState<string | null>(null);
  const [buffer, setBuffer] = useState("");
  const [params, setParams] = useState<RuleParam[]>([]);
  const [savedBody, setSavedBody] = useState<string | null>(null);
  // The persisted params snapshot, for the dirty check (a param edit alone should enable Save).
  const [savedParams, setSavedParams] = useState<string | null>(null);
  const [result, setResult] = useState<RunResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [hasRun, setHasRun] = useState(false);

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
    setHasRun(false);
    try {
      const rule = await getRule(id);
      setSelectedId(rule.id);
      setName(rule.name);
      setBuffer(rule.body);
      setParams(rule.params ?? []);
      setSavedBody(rule.body);
      setSavedParams(JSON.stringify(rule.params ?? []));
    } catch (e) {
      setError(msg(e));
    }
  }, []);

  const newRule = useCallback(() => {
    setSelectedId(null);
    setName(null);
    setBuffer("");
    setParams([]);
    setSavedBody(null);
    setSavedParams(null);
    setResult(null);
    setError(null);
    setHasRun(false);
  }, []);

  const create = useCallback(
    async (nameArg: string): Promise<string | null> => {
      const trimmed = nameArg.trim();
      if (!trimmed) return null;
      const id = uniqueId(slugify(trimmed), new Set(roster.map((r) => r.id)));
      setError(null);
      try {
        // Save what's currently in the buffer under the new name — "save what I'm working on as a new
        // rule." Empty buffer is fine (creates a starter rule the user then edits + Saves normally).
        await saveRule({ id, name: trimmed, body: buffer, params });
        setSelectedId(id);
        setName(trimmed);
        setSavedBody(buffer);
        setSavedParams(JSON.stringify(params));
        await refresh();
        return id;
      } catch (e) {
        setError(msg(e));
        return null;
      }
    },
    [buffer, params, roster, refresh],
  );

  const rename = useCallback(
    async (newName: string): Promise<boolean> => {
      const id = selectedId;
      const trimmed = newName.trim();
      if (!id || !trimmed) return false;
      setError(null);
      try {
        // Re-save the SAME id with the new name + the persisted body + params (rename edits neither).
        await saveRule({ id, name: trimmed, body: savedBody ?? buffer, params });
        setName(trimmed);
        setSavedParams(JSON.stringify(params));
        await refresh();
        return true;
      } catch (e) {
        setError(msg(e));
        return false;
      }
    },
    [selectedId, savedBody, buffer, params, refresh],
  );

  const dirty =
    savedBody !== null && (buffer !== savedBody || JSON.stringify(params) !== savedParams);

  const loadExample = useCallback(
    (body: string) => {
      // Respect the dirty indicator: a fresh buffer with unsaved edits isn't silently clobbered.
      const hasUnsaved = buffer.trim() !== "" && (savedBody === null || buffer !== savedBody);
      if (hasUnsaved && typeof window !== "undefined") {
        const ok = window.confirm("Replace the editor with this example? Unsaved edits will be lost.");
        if (!ok) return;
      }
      // An example is an ad-hoc body — detach from any open saved rule (and its params).
      setSelectedId(null);
      setSavedBody(null);
      setSavedParams(null);
      setBuffer(body);
      setParams([]);
      setResult(null);
      setError(null);
      setHasRun(false);
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
      setHasRun(true);
    }
  }, [buffer]);

  const save = useCallback(
    async (id: string, name: string) => {
      setError(null);
      try {
        await saveRule({ id, name, body: buffer, params });
        setSelectedId(id);
        setSavedBody(buffer);
        setSavedParams(JSON.stringify(params));
        await refresh();
      } catch (e) {
        setError(msg(e));
      }
    },
    [buffer, params, refresh],
  );

  const saveCurrent = useCallback(
    async (nameForNew?: string): Promise<{ ok: boolean; needsName: boolean }> => {
      // A saved rule is open → update it in place under its existing id + name.
      if (selectedId) {
        await save(selectedId, name ?? selectedId);
        return { ok: true, needsName: false };
      }
      // Fresh ad-hoc buffer → it needs a name before it can become a saved rule.
      const trimmed = nameForNew?.trim();
      if (!trimmed) return { ok: false, needsName: true };
      const id = await create(trimmed);
      return { ok: id !== null, needsName: false };
    },
    [selectedId, name, save, create],
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
    name,
    buffer,
    params,
    savedBody,
    result,
    error,
    running,
    dirty,
    setBuffer,
    setParams,
    refresh,
    open,
    newRule,
    create,
    rename,
    loadExample,
    run,
    save,
    saveCurrent,
    hasRun,
    remove,
  };
}
