// The reusable command palette (channels-command-palette scope) — the channel input as a COMMAND
// SURFACE, not a chat box. `/` at line start opens the catalog (cached, 0ms — no network in the open
// path); accepting a tool drives an argument rail from its JSON Schema; an `x-lb-entity` arg
// auto-opens the `@`-picker, an `x-lb-widget:sql` arg opens the mini SQL editor. Picked entities are
// CHIP tokens that delete whole (one ⌫ removes a whole chip, never half a name). On submit it emits
// a STRUCTURED payload — a `kind:"query"` channel Item for `federation.query`, else a `{tool,args}`
// bridge call — never raw `/`-text. Keyboard-driven: / @ ↑ ↓ ⏎ ⌫ Esc Tab.
//
// This file is the controller + render; the cached catalog (`useCatalog`), entity listers
// (`useMentions`), the pure parser (`parsePalette`), and the widgets live in their own files
// (FILE-LAYOUT). Data fetching is delegated; the post/dispatch is the parent's `onPostQuery` /
// `onCallTool` / `onSendChat` (the existing `channel.post` / `mcp_call` seams).

import { useMemo, useState } from "react";
import { SendHorizontal } from "lucide-react";

import { argNames, hintFor } from "@/lib/channel/palette.types";
import type { ToolDescriptor } from "@/lib/channel/palette.types";
import { useCatalog } from "./useCatalog";
import { useMentions } from "./useMentions";
import { parsePalette } from "./parsePalette";
import { EntityPicker } from "./argWidgets/EntityPicker";
import { SqlArg } from "./argWidgets/SqlArg";

interface Props {
  channel: string;
  /** Emit a `kind:"query"` channel Item (federation.query's structured payload). */
  onPostQuery: (source: string, sql: string) => void | Promise<void>;
  /** Dispatch any other tool via the host-mediated bridge. */
  onCallTool: (tool: string, args: Record<string, unknown>) => void | Promise<void>;
  /** Send a plain chat message (no command). */
  onSendChat: (body: string) => void | Promise<void>;
}

/** One filled argument (a chip). `entity` chips delete whole; the SQL arg is edited inline. */
interface ArgChip {
  name: string;
  value: string;
}

export function CommandPalette({ channel, onPostQuery, onCallTool, onSendChat }: Props) {
  const { tools, error: catalogError } = useCatalog();
  const [text, setText] = useState("");
  // Once a tool is accepted we leave free-typing and fill its args one at a time.
  const [tool, setTool] = useState<ToolDescriptor | null>(null);
  const [chips, setChips] = useState<ArgChip[]>([]);
  const [argText, setArgText] = useState(""); // the in-progress free-text/entity-query for the active arg
  const [sql, setSql] = useState("");
  const [sel, setSel] = useState(0);

  // The arg rail: the ordered arg names, and the one we're currently filling. An arg is "satisfied"
  // when it has a chip (entity/text args) — the sql arg is filled inline and stays active until
  // submit. The active arg is the first unsatisfied one.
  const args = useMemo(() => argNames(tool?.input_schema), [tool]);
  const filledNames = useMemo(() => new Set(chips.map((c) => c.name)), [chips]);
  const activeArg = tool
    ? args.find((a) => hintFor(tool.input_schema, a)?.widget === "sql" || !filledNames.has(a))
    : undefined;
  const activeHint = activeArg ? hintFor(tool?.input_schema, activeArg) : undefined;
  const entity = activeHint?.entity ?? null;
  const isSqlArg = activeHint?.widget === "sql";

  // The chosen `source` chip (drives SQL schema autocomplete) — the first `entity:datasource` chip.
  const source = chips.find((c) => c.name === "source")?.value ?? null;

  const mentions = useMentions(tool && entity ? entity : null);
  const filtered = useMemo(
    () => (entity ? parsePalette("@" + argText, [], mentions.items).candidates : []),
    [entity, argText, mentions.items],
  );

  // Command-mode menu (no tool chosen yet).
  const parse = useMemo(() => parsePalette(text, tools), [text, tools]);
  const showCommandMenu = tool === null && parse.mode === "command";

  function reset() {
    setText("");
    setTool(null);
    setChips([]);
    setArgText("");
    setSql("");
    setSel(0);
  }

  function acceptTool(d: ToolDescriptor) {
    setTool(d);
    setText("");
    setChips([]);
    setArgText("");
    setSql("");
    setSel(0);
  }

  function pickEntity(value: string) {
    if (!activeArg) return;
    setChips((c) => [...c, { name: activeArg, value }]);
    setArgText("");
    setSel(0);
  }

  function removeLastChip() {
    setChips((c) => c.slice(0, -1));
    setArgText("");
  }

  async function submit() {
    if (tool === null) {
      // No command — a plain chat message (parse.mode === chat). Ignore a half-typed `/cmd`.
      if (text.trim() && !text.startsWith("/")) await onSendChat(text.trim());
      reset();
      return;
    }
    // Build the args object from the chips + the SQL field.
    const argsObj: Record<string, unknown> = {};
    for (const chip of chips) argsObj[chip.name] = chip.value;
    if (args.includes("sql")) argsObj.sql = sql;
    if (tool.name === "federation.query" && typeof argsObj.source === "string") {
      await onPostQuery(argsObj.source, String(argsObj.sql ?? ""));
    } else {
      await onCallTool(tool.name, argsObj);
    }
    reset();
  }

  // The top-level free-text input keyboard map (command mode + chat).
  function onInputKey(e: React.KeyboardEvent) {
    if (!showCommandMenu) {
      if (e.key === "Enter") {
        e.preventDefault();
        void submit();
      }
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSel((s) => Math.min(s + 1, parse.candidates.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSel((s) => Math.max(s - 1, 0));
    } else if (e.key === "Enter" || e.key === "Tab") {
      e.preventDefault();
      const pick = parse.candidates[sel] ?? parse.candidates[0];
      if (pick) acceptTool(tools.find((t) => t.name === pick.value)!);
    } else if (e.key === "Escape") {
      e.preventDefault();
      reset();
    }
  }

  // The entity-picker keyboard map (mention mode).
  function onArgKey(e: React.KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSel((s) => Math.min(s + 1, filtered.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSel((s) => Math.max(s - 1, 0));
    } else if (e.key === "Enter" || e.key === "Tab") {
      e.preventDefault();
      const pick = filtered[sel] ?? filtered[0];
      if (pick) pickEntity(pick.value);
    } else if (e.key === "Backspace" && argText === "" && chips.length > 0) {
      e.preventDefault();
      removeLastChip(); // one ⌫ deletes the whole chip
    } else if (e.key === "Escape") {
      e.preventDefault();
      reset();
    }
  }

  const allArgsFilled = tool !== null && !activeArg && !isSqlArg;

  return (
    <div className="border-t border-border bg-panel/70">
      {catalogError && (
        <div role="alert" className="px-3 py-1 text-xs text-red-600 dark:text-red-300">
          {catalogError}
        </div>
      )}

      {/* The command menu (cached catalog — opens with no network). */}
      {showCommandMenu && (
        <ul role="listbox" aria-label="commands" className="max-h-56 overflow-y-auto py-1">
          {parse.candidates.length === 0 ? (
            <li role="note" className="px-3 py-2 text-xs text-muted">
              No commands match
            </li>
          ) : (
            parse.candidates.map((c, i) => (
              <li
                key={c.value}
                role="option"
                aria-selected={i === sel}
                onMouseEnter={() => setSel(i)}
                onMouseDown={(e) => {
                  e.preventDefault();
                  acceptTool(tools.find((t) => t.name === c.value)!);
                }}
                className={`flex cursor-pointer items-baseline gap-2 px-3 py-1.5 text-sm ${
                  i === sel ? "bg-accent/15 text-fg" : "text-fg"
                }`}
              >
                <span className="font-medium">/{c.label}</span>
                {c.hint && <span className="text-xs text-muted">{c.hint}</span>}
              </li>
            ))
          )}
        </ul>
      )}

      {/* The arg rail — the chosen tool + its filled chips. */}
      {tool && (
        <div className="flex flex-wrap items-center gap-1.5 px-3 pt-2" aria-label="command args">
          <span className="rounded-md bg-accent/15 px-2 py-0.5 text-xs font-medium text-accent">
            /{tool.title || tool.name}
          </span>
          {chips.map((chip, i) => (
            <span
              key={`${chip.name}-${i}`}
              data-testid="arg-chip"
              className="flex items-center gap-1 rounded-md border border-border bg-bg px-2 py-0.5 text-xs"
            >
              <span className="text-muted">{chip.name}:</span>
              <span className="font-medium">{chip.value}</span>
            </span>
          ))}
        </div>
      )}

      {/* The active entity picker. */}
      {tool && entity && (
        <div onKeyDownCapture={onArgKey}>
          <input
            aria-label={`@${activeArg}`}
            value={argText}
            autoFocus
            onChange={(e) => {
              setArgText(e.target.value.replace(/^@/, ""));
              setSel(0);
            }}
            onKeyDown={onArgKey}
            placeholder={`@ pick a ${activeArg}`}
            className="control-field mx-3 my-2 w-[calc(100%-1.5rem)]"
          />
          <EntityPicker
            arg={activeArg!}
            candidates={filtered}
            selected={sel}
            reason={mentions.reason}
            loading={mentions.loading}
            onPick={pickEntity}
            onMove={setSel}
          />
        </div>
      )}

      {/* The active SQL widget. */}
      {tool && isSqlArg && (
        <SqlArg
          source={source}
          kind="sqlite"
          value={sql}
          onChange={setSql}
          onSubmit={() => void submit()}
          onCancel={reset}
        />
      )}

      {/* The free-text input — command/chat entry, plus the submit affordance when args are done. */}
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
        className="flex items-center gap-2 p-3"
      >
        {tool === null && (
          <input
            aria-label="message"
            value={text}
            onChange={(e) => {
              setText(e.target.value);
              setSel(0);
            }}
            onKeyDown={onInputKey}
            placeholder={`Message #${channel}  ·  / for commands`}
            className="control-field min-w-0 flex-1"
          />
        )}
        {allArgsFilled && (
          <span className="flex-1 text-xs text-muted">Ready — press send to run</span>
        )}
        <button
          type="submit"
          aria-label="send"
          disabled={tool === null ? !text.trim() : isSqlArg && !sql.trim()}
          className="soft-button h-9 px-3"
        >
          <SendHorizontal size={16} />
        </button>
      </form>
    </div>
  );
}
