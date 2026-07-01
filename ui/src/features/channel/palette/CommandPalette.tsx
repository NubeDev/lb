// The reusable command palette (channels-command-palette scope) — the channel input as a COMMAND
// SURFACE. `/` opens the cached catalog; accepting a tool drives an argument rail from its JSON Schema.
// An `x-lb-entity` arg auto-opens the `@`-picker; every other arg resolves through the x-lb WIDGET
// REGISTRY (sql/runtime/select/cron/boolean/number/date/ext:<id>/<widget>, or the text fallback — an
// unknown widget never crashes). Picked entities are CHIP tokens (one ⌫ deletes a whole chip). On submit
// it is TOOL-AGNOSTIC: a descriptor that DECLARES a `result` render-envelope has that render POSTED (with
// the collected args interpolated into `source.args`); everything else is a plain `{tool,args}` bridge
// call. It has ZERO tool-specific knowledge — no branch on any tool NAME for the generic path.
//
// (`agent.invoke` → `kind:"agent"` and `federation.query` → `kind:"query"` are the two shipped
// first-class payload routes kept as-is; converting them to descriptor-declared routes like the generic
// `result` path is a named follow-up, not this pass.)
//
// Controller + render only; the catalog (`useCatalog`), listers (`useMentions`), parser (`parsePalette`),
// and arg widgets (`argWidgets/` + `argWidgets/registry`) live in their own files (FILE-LAYOUT). The
// post/dispatch is the parent's on*/onPostRich seams.

import { useMemo, useState } from "react";
import { SendHorizontal } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { argNames, hintFor } from "@/lib/channel/palette.types";
import type { ToolDescriptor } from "@/lib/channel/palette.types";
import { encodeRichResult } from "@/lib/channel/payload.types";
import { useCatalog } from "./useCatalog";
import { useMentions } from "./useMentions";
import { parsePalette } from "./parsePalette";
import { EntityPicker } from "./argWidgets/EntityPicker";
import { ActiveArgWidget } from "./argWidgets/ActiveArgWidget";
import { resolveWidget } from "./argWidgets/registry";

interface Props {
  channel: string;
  /** Emit a `kind:"query"` channel Item (federation.query's structured payload). */
  onPostQuery: (source: string, sql: string) => void | Promise<void>;
  /** Post a `kind:"agent"` request (the agent.invoke command's payload path — NOT a raw tool call).
   *  `runtime` selects the agent (the picker's default id → the in-house default). */
  onSendAgent: (goal: string, runtime?: string) => void | Promise<void>;
  /** Dispatch any other tool via the host-mediated bridge. */
  onCallTool: (tool: string, args: Record<string, unknown>) => void | Promise<void>;
  /** Post a pre-encoded `kind:"rich_result"` render-envelope Item (a descriptor-declared interactive
   *  response — the palette posts whatever render the tool's descriptor declared, tool-agnostic). */
  onPostRich: (body: string) => void | Promise<void>;
  /** Send a plain chat message (no command). */
  onSendChat: (body: string) => void | Promise<void>;
}

/** One filled argument (a chip). `entity`/text/number/date chips delete whole; inline widgets stay live. */
interface ArgChip {
  name: string;
  value: string;
}

export function CommandPalette({
  channel,
  onPostQuery,
  onSendAgent,
  onCallTool,
  onPostRich,
  onSendChat,
}: Props) {
  const { tools, error: catalogError } = useCatalog();
  const [text, setText] = useState("");
  // Once a tool is accepted we leave free-typing and fill its args one at a time.
  const [tool, setTool] = useState<ToolDescriptor | null>(null);
  const [chips, setChips] = useState<ArgChip[]>([]);
  const [argText, setArgText] = useState(""); // the in-progress free-text/entity-query for the active arg
  // Inline-widget values keyed by arg name (sql/runtime/select/cron/boolean stay live until submit).
  const [inlineVals, setInlineVals] = useState<Record<string, string>>({});
  const [sel, setSel] = useState(0);

  // The arg rail: the ordered arg names, and the one we're currently filling. An arg is "satisfied" when
  // it has a chip (chip widgets) — an INLINE widget (sql/runtime/select/cron/boolean) is filled in place
  // and stays active until submit. The active arg is the first unsatisfied chip arg, INLINE widgets last
  // (so a required text arg like the agent's `goal` is filled before the runtime dropdown takes focus).
  const args = useMemo(() => argNames(tool?.input_schema), [tool]);
  const filledNames = useMemo(() => new Set(chips.map((c) => c.name)), [chips]);
  const widgetOf = (a: string) => resolveWidget(hintFor(tool?.input_schema, a));
  const isInline = (a: string) => widgetOf(a).inline;
  const activeArg = tool
    ? // a non-inline unfilled arg first; only when all of those are filled does an inline widget go active
      args.find((a) => !isInline(a) && !filledNames.has(a)) ?? args.find((a) => isInline(a))
    : undefined;
  const activeHint = activeArg ? hintFor(tool?.input_schema, activeArg) : undefined;
  const activeWidget = activeArg ? widgetOf(activeArg) : undefined;
  const entity = activeHint?.entity ?? null;
  // Entity args keep the shared @-picker (↑↓⏎ mention-nav); every other active arg renders via the widget
  // registry (ActiveArgWidget). An inline widget stays live until submit; a chip widget commits to a chip.
  const showEntityPicker = !!activeArg && !!entity;
  const showArgWidget = !!activeArg && !entity;
  const isInlineActive = !!activeWidget?.inline;

  // The chosen `source` chip (drives SQL schema autocomplete) — the first `entity:datasource` chip.
  const source = chips.find((c) => c.name === "source")?.value ?? null;
  // The active arg's live value: an inline widget reads its per-arg inline value; a chip widget reads
  // the in-progress `argText`.
  const activeValue = activeArg ? (isInlineActive ? inlineVals[activeArg] ?? "" : argText) : "";

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
    setInlineVals({});
    setSel(0);
  }

  function acceptTool(d: ToolDescriptor) {
    setTool(d);
    setText("");
    setChips([]);
    setArgText("");
    setInlineVals({});
    setSel(0);
  }

  /** Set the active arg's live value — into the inline map (inline widget) or `argText` (chip widget). */
  function setActiveValue(v: string) {
    if (activeArg && isInlineActive) setInlineVals((m) => ({ ...m, [activeArg]: v }));
    else setArgText(v);
  }

  // Commit the in-progress text into a chip for the active chip arg (e.g. the agent's `goal`).
  function commitTextArg() {
    if (!activeArg || !argText.trim()) return;
    setChips((c) => [...c, { name: activeArg, value: argText.trim() }]);
    setArgText("");
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

  /** The full args object: chips + every inline widget value + a chip arg still being typed (folded). */
  function buildArgs(): Record<string, string> {
    const argsObj: Record<string, string> = {};
    for (const chip of chips) argsObj[chip.name] = chip.value;
    for (const a of args) if (isInline(a) && inlineVals[a] !== undefined) argsObj[a] = inlineVals[a];
    if (activeArg && !isInlineActive && argText.trim() && !(activeArg in argsObj)) {
      argsObj[activeArg] = argText.trim();
    }
    return argsObj;
  }

  async function submit() {
    if (tool === null) {
      // No command — a plain chat message (parse.mode === chat). Ignore a half-typed `/cmd`.
      if (text.trim() && !text.startsWith("/")) await onSendChat(text.trim());
      reset();
      return;
    }
    const argsObj = buildArgs();

    // TODO(follow-up): make agent/query descriptor-declared routes like the generic result path.
    // The agent command is a FIRST-CLASS palette command routed to the `kind:"agent"` payload path
    // (`onSendAgent`); federation.query routes to `onPostQuery`. Both are shipped/green and stay as-is.
    if (tool.name === "agent.invoke") {
      const goal = String(argsObj.goal ?? "");
      const runtime = argsObj.runtime;
      if (goal.trim()) await onSendAgent(goal.trim(), runtime || undefined);
    } else if (tool.name === "federation.query" && typeof argsObj.source === "string") {
      await onPostQuery(argsObj.source, String(argsObj.sql ?? ""));
    } else if (tool.result) {
      // The descriptor DECLARES a render — POST it, tool-agnostic. The collected args are interpolated
      // into `source.args` (a shallow merge over the descriptor's own args), so a list command's filters
      // (status/limit/…) reach the re-run source. ZERO tool knowledge: any descriptor.result flows here.
      const render = {
        ...tool.result,
        source: tool.result.source
          ? { ...tool.result.source, args: { ...tool.result.source.args, ...argsObj } }
          : tool.result.source,
      };
      await onPostRich(encodeRichResult(render));
    } else {
      // A plain bridge call — the collected form fields VERBATIM (the backend verb owns any reshaping).
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

  // "Ready to run": every chip arg is filled and the active arg (if any) is an always-ready inline widget.
  const allArgsFilled = tool !== null && (!activeArg || isInlineActive);
  // Disabled only when a REQUIRED chip widget is still empty (the sql widget needs text; a runtime/
  // select/cron/boolean is always runnable).
  const submitDisabled =
    tool === null
      ? !text.trim()
      : activeWidget?.kind === "sql"
        ? !activeValue.trim()
        : !!activeArg && !isInlineActive && chips.every((c) => c.name !== activeArg) && !argText.trim();

  return (
    <div className="border-t border-border bg-panel/70">
      {catalogError && (
        <div role="alert" className="px-3 py-1 text-xs text-destructive">
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

      {/* The active entity picker (keeps the shared @-mention keyboard nav). */}
      {tool && showEntityPicker && (
        <div onKeyDownCapture={onArgKey}>
          <Input
            aria-label={`@${activeArg}`}
            value={argText}
            autoFocus
            onChange={(e) => {
              setArgText(e.target.value.replace(/^@/, ""));
              setSel(0);
            }}
            onKeyDown={onArgKey}
            placeholder={`@ pick a ${activeArg}`}
            className="mx-3 my-2 w-[calc(100%-1.5rem)]"
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

      {/* Every other active arg — resolved through the x-lb widget registry. A chip widget's ⏎ commits
          the chip (or submits when it is the last arg); an inline widget stays live until submit. */}
      {tool && showArgWidget && activeArg && (
        <div
          onKeyDownCapture={(e) => {
            if (e.key === "Backspace" && argText === "" && !isInlineActive && chips.length > 0) {
              e.preventDefault();
              removeLastChip();
            }
          }}
        >
          <ActiveArgWidget
            name={activeArg}
            hint={activeHint}
            value={activeValue}
            onChange={setActiveValue}
            onSubmit={() => {
              // A chip widget's ⏎: submit if it is the last arg (nothing inline follows), else chip it.
              const moreArgsFollow = args.some((a) => a !== activeArg && isInline(a));
              if (!moreArgsFollow) void submit();
              else commitTextArg();
            }}
            onCancel={reset}
            sqlSource={source}
          />
        </div>
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
          <Input
            aria-label="message"
            value={text}
            onChange={(e) => {
              setText(e.target.value);
              setSel(0);
            }}
            onKeyDown={onInputKey}
            placeholder={`Message #${channel}  ·  / for commands`}
            className="min-w-0 flex-1"
          />
        )}
        {allArgsFilled && (
          <span className="flex-1 text-xs text-muted">Ready — press send to run</span>
        )}
        <Button type="submit" aria-label="send" disabled={submitDisabled} className="px-3">
          <SendHorizontal size={16} />
        </Button>
      </form>
    </div>
  );
}
