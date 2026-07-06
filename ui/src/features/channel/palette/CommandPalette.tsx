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

import { useEffect, useMemo, useState } from "react";
import { SendHorizontal } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { argNames, hintFor, isActiveRequired, isShown } from "@/lib/channel/palette.types";
import type { ToolDescriptor } from "@/lib/channel/palette.types";
import { encodeRichResult } from "@/lib/channel/payload.types";
import { resolveWidget, resolveFieldPresentation } from "@/lib/widgets";
import { useCatalog } from "./useCatalog";
import { useMentions } from "./useMentions";
import { parsePalette } from "./parsePalette";
import { EntityPicker } from "./argWidgets/EntityPicker";
import { ActiveArgWidget } from "./argWidgets/ActiveArgWidget";

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
  /** Optional PREFILL (query re-edit scope): open `tool` with `args` pre-seeded — a chip arg becomes a
   *  chip, an inline arg (sql/…) seeds its live widget value — so "edit this query" skips the whole
   *  source-reselection walk. Consumed once via `onPrefillConsumed` (the parent clears it). */
  prefill?: PalettePrefill | null;
  onPrefillConsumed?: () => void;
}

/** A one-shot "open this tool with these args" request (see `prefill`). */
export interface PalettePrefill {
  tool: string;
  args: Record<string, string>;
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
  prefill,
  onPrefillConsumed,
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
  // An arg is "filled" once it has a chip (chip widgets) OR a non-empty inline value (inline widgets —
  // sql/select/cron/…). Inline args were previously NEVER counted filled (chip-only), so a form with
  // TWO required inline widgets (e.g. `/remind`'s cron `schedule` + `select` `action_kind`) stuck on the
  // first one forever — the second widget never activated and its arg was dropped from the call
  // ("missing string arg: action_kind"). Counting a valued inline arg as filled lets the rail advance
  // through each required inline widget in turn.
  const filledNames = useMemo(() => {
    const s = new Set(chips.map((c) => c.name));
    for (const [k, v] of Object.entries(inlineVals)) if (v !== undefined && v !== "") s.add(k);
    return s;
  }, [chips, inlineVals]);
  // The currently-collected form VALUES (chips + inline widget values) — the input to conditional
  // visibility (`x-lb.showIf`). A `select` like `action_kind` writes into `inlineVals`, so switching it
  // re-computes which per-kind fields are shown/required. (The in-progress chip text is folded at submit,
  // not here — a half-typed value shouldn't flip a condition mid-keystroke.)
  const values = useMemo(() => {
    const v: Record<string, string> = {};
    for (const c of chips) v[c.name] = c.value;
    for (const [k, val] of Object.entries(inlineVals)) if (val !== undefined) v[k] = val;
    return v;
  }, [chips, inlineVals]);
  const widgetOf = (a: string) => resolveWidget(hintFor(tool?.input_schema, a));
  const isInline = (a: string) => widgetOf(a).inline;
  // ACTIVE-required = unconditionally `required`, or currently-shown AND `requiredWhenShown` (a
  // conditional per-kind field, e.g. `channel` once `action_kind=channel-post`). This is what makes the
  // rail walk `required ∪ shown-and-required` and surface the action fields Bug B left unreachable.
  const required = (a: string) => isActiveRequired(tool?.input_schema, a, values);
  const shown = (a: string) => isShown(tool?.input_schema, a, values);
  // The rail auto-targets ACTIVE-required unfilled CHIP args only. An optional arg NEVER grabs the
  // single active slot — so the required `goal` stays the field you type into, and an all-optional-
  // filters command is runnable the instant it is picked. INLINE widgets never take the active slot
  // (they render persistently below) — routing a required inline arg through the slot was the
  // "sql editor closes on the first keystroke" bug: one typed char made it count as FILLED, the slot
  // moved on, and the editor UNMOUNTED mid-typing with no way back.
  const activeArg = tool
    ? args.find((a) => required(a) && !isInline(a) && !filledNames.has(a))
    : undefined;
  // ALL INLINE widgets (required and optional) render PERSISTENTLY, alongside the active chip arg —
  // not through the single active slot (which only holds one arg at a time). This is what the "no
  // runtime picker" bug got wrong for optional inline (the runtime dropdown only appeared after the
  // goal was committed), and what the sql-unmount bug got wrong for required inline (typing = filled
  // = unmounted). A per-kind inline field still only shows once its `showIf` matches (`shown`).
  const inlineArgs = tool ? args.filter((a) => isInline(a) && shown(a)) : [];
  const activeHint = activeArg ? hintFor(tool?.input_schema, activeArg) : undefined;
  // The resolved PRESENTATION for the active arg (widget-kit scope) — the SAME `resolveFieldPresentation`
  // the table headers funnel through, reading the form's `x-lb` label/description (humanize fallback). So
  // a field's rail label and its table header never drift (`max_runs` → "Max Runs" in both).
  const activePresentation = activeArg ? resolveFieldPresentation(activeArg, activeHint) : undefined;
  const entity = activeHint?.entity ?? null;
  // Entity args keep the shared @-picker (↑↓⏎ mention-nav); every other active arg renders via the widget
  // registry (ActiveArgWidget). An inline widget stays live until submit; a chip widget commits to a chip.
  const showEntityPicker = !!activeArg && !!entity;
  const showArgWidget = !!activeArg && !entity;

  // The chosen `source` chip (drives SQL schema autocomplete) — the first `entity:datasource` chip.
  const source = chips.find((c) => c.name === "source")?.value ?? null;
  // The active arg's live value — always a chip arg now (inline widgets read `inlineVals` directly).
  const activeValue = activeArg ? argText : "";

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

  // PREFILL (query re-edit): open the requested tool with its args pre-seeded — a chip arg becomes a
  // chip, an inline arg (sql/…) seeds its live widget — so "edit this query" reopens the palette past
  // the whole selection walk. One-shot: consumed as soon as the catalog can resolve the tool name.
  useEffect(() => {
    if (!prefill) return;
    const d = tools.find((t) => t.name === prefill.tool);
    if (!d) return; // catalog still loading (or the tool isn't granted) — retry when `tools` lands
    setTool(d);
    setText("");
    setArgText("");
    setSel(0);
    const seededChips: ArgChip[] = [];
    const seededInline: Record<string, string> = {};
    for (const [name, value] of Object.entries(prefill.args)) {
      if (resolveWidget(hintFor(d.input_schema, name)).inline) seededInline[name] = value;
      else seededChips.push({ name, value });
    }
    setChips(seededChips);
    setInlineVals(seededInline);
    onPrefillConsumed?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [prefill, tools]);

  /** Set the active chip arg's in-progress text (inline widgets write `inlineVals` directly). */
  function setActiveValue(v: string) {
    setArgText(v);
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

  /** The full args object: chips + every inline widget value + a chip arg still being typed (folded).
   *  HIDDEN fields are dropped: a per-kind field whose `x-lb.showIf` no longer matches (e.g. a `channel`
   *  typed under `action_kind=channel-post`, then switched to `outbox`) must NOT leak a stale value into
   *  the call — the verb would build the wrong action. Only currently-shown fields are collected. */
  function buildArgs(): Record<string, string> {
    const argsObj: Record<string, string> = {};
    for (const chip of chips) if (shown(chip.name)) argsObj[chip.name] = chip.value;
    for (const a of args) if (isInline(a) && inlineVals[a] !== undefined && shown(a)) argsObj[a] = inlineVals[a];
    if (activeArg && argText.trim() && !(activeArg in argsObj)) {
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

  // A required, shown INLINE widget that is still empty (e.g. the sql editor before any typing) —
  // the command is not runnable yet, but the widget STAYS MOUNTED (it never cycles through the slot).
  const emptyRequiredInline = inlineArgs.some(
    (a) => required(a) && !(inlineVals[a] !== undefined && inlineVals[a] !== ""),
  );
  // "Ready to run": every required chip arg is filled AND every required inline widget has a value.
  const allArgsFilled = tool !== null && !activeArg && !emptyRequiredInline;
  // Disabled while a REQUIRED chip arg has neither a chip nor in-progress text, or a required inline
  // widget (sql/cron/…) is still empty.
  const submitDisabled =
    tool === null
      ? !text.trim()
      : (!!activeArg && chips.every((c) => c.name !== activeArg) && !argText.trim()) ||
        emptyRequiredInline;

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

      {/* Every other active CHIP arg — resolved through the x-lb widget registry. Its ⏎ commits the
          chip (or submits when nothing inline follows). Inline widgets render persistently below. */}
      {tool && showArgWidget && activeArg && (
        <div
          onKeyDownCapture={(e) => {
            if (e.key === "Backspace" && argText === "" && chips.length > 0) {
              e.preventDefault();
              removeLastChip();
            }
          }}
        >
          {/* The resolved presentation label + help — same resolver the table headers use, so a form
              field's label and its table header agree (widget-kit scope). */}
          {activePresentation && (
            <div className="px-3 pt-2" aria-label={`field ${activeArg}`}>
              <span className="text-xs font-medium text-fg">{activePresentation.label}</span>
              {activePresentation.description && (
                <span className="ml-2 text-xs text-muted">{activePresentation.description}</span>
              )}
            </div>
          )}
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

      {/* ALL INLINE widgets — rendered PERSISTENTLY beside the active chip arg (never through the
          single active slot). This is how the agent command's `runtime` picker shows the moment
          `/agent` is picked, and how the REQUIRED sql editor stays mounted while you type (the
          slot-cycling unmount was the "sql editor closes on the first keystroke" bug). Each shows
          its resolved presentation label and writes into the same per-arg inline map (`inlineVals`)
          the args object folds in at submit. */}
      {tool &&
        inlineArgs.map((a) => {
          const hint = hintFor(tool.input_schema, a);
          const presentation = resolveFieldPresentation(a, hint);
          return (
            <div key={a}>
              <div className="px-3 pt-2" aria-label={`field ${a}`}>
                <span className="text-xs font-medium text-fg">{presentation.label}</span>
                {presentation.description && (
                  <span className="ml-2 text-xs text-muted">{presentation.description}</span>
                )}
              </div>
              <ActiveArgWidget
                name={a}
                hint={hint}
                value={inlineVals[a] ?? ""}
                onChange={(v) => setInlineVals((m) => ({ ...m, [a]: v }))}
                onSubmit={() => void submit()}
                onCancel={reset}
                sqlSource={source}
              />
            </div>
          );
        })}

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
