// The JSON payload builder (widget-config-vars Slice 5). A CodeMirror JSON editor authoring a template
// with `${var}` / `{{value}}` slots, plus a TARGET picker (an extension write tool via the source
// picker's Action group, `bus.publish`, or `ingest.write`). On send: parse the template → run it through
// the shared `interpolateArgs(template, scope)` → `bridge.call(target, payload)` (leashed by the cell's
// tool set ∩ grant; the host re-checks). Reuses the Slice-B CodeEditor (JS mode highlights JSON).
//
// State vs motion (rule 3): a `bus.publish` is fire-and-forget — the UI shows "published" (handed to the
// bus), NEVER a fake "delivered". A real write tool shows "sent" only after the host accepts it.

import { useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import type { VarScope } from "@/lib/vars";
import { interpolateArgs, emptyScope } from "@/lib/vars";
import type { ExtRow } from "@/lib/ext/ext.api";
import { CodeEditor } from "./editors/CodeEditor";
import { makeWidgetBridge } from "./widgetBridge";
import { extensionEntries } from "./sourcePicker";

/** A send target — a write tool the payload is delivered to. `bus.publish`/`ingest.write` are always
 *  offered; an installed extension's write tools come from `ext.list` (the source picker's Action group). */
export interface PayloadTarget {
  tool: string;
  label: string;
}

const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

const DEFAULT_TEMPLATE = '{\n  "text": "${newTodo}",\n  "ws": "${__workspace}"\n}';

/** The targets a payload may be sent to: the platform sinks + every installed extension write tool. */
export function payloadTargets(installed: ExtRow[]): PayloadTarget[] {
  const out: PayloadTarget[] = [
    { tool: "bus.publish", label: "Over the bus (bus.publish)" },
    { tool: "ingest.write", label: "Ingest a sample (ingest.write)" },
  ];
  for (const e of extensionEntries(installed)) {
    if (e.writes && e.action?.tool) out.push({ tool: e.action.tool, label: e.label });
  }
  return out;
}

interface Props {
  ws: string;
  installed: ExtRow[];
  /** The resolved variable scope — `${var}`/`{{value}}` interpolate against it before send. */
  scope?: VarScope;
}

type Status = { kind: "idle" | "ok" | "error"; msg?: string };

/** The authoring + send surface. Standalone (a button control's JSON body); the cell's tool set leash is
 *  the chosen target + bridge re-check. */
export function JsonPayloadField({ ws, installed, scope = emptyScope() }: Props) {
  const targets = useMemo(() => payloadTargets(installed), [installed]);
  const [target, setTarget] = useState<string>(targets[0]?.tool ?? "bus.publish");
  const [subject, setSubject] = useState<string>("ui/banner");
  const [template, setTemplate] = useState<string>(DEFAULT_TEMPLATE);
  const [status, setStatus] = useState<Status>({ kind: "idle" });

  const isPublish = target === "bus.publish";

  async function send() {
    setStatus({ kind: "idle" });
    let parsed: unknown;
    try {
      parsed = JSON.parse(template);
    } catch (e) {
      setStatus({ kind: "error", msg: `invalid JSON: ${e instanceof Error ? e.message : e}` });
      return;
    }
    // Interpolate the template against the scope (type-preserving) BEFORE the call.
    const payload = interpolateArgs(parsed, scope) as Record<string, unknown>;
    // bus.publish wraps the payload as `{ subject, payload }`; other tools take the payload directly.
    const args = isPublish ? { subject, payload } : payload;
    // The bridge is leashed to the chosen target; the host re-checks the cap + workspace per call.
    const bridge = makeWidgetBridge([target]);
    try {
      await bridge.call(target, args);
      // A publish is fire-and-forget — say "published", never "delivered" (rule 3).
      setStatus({ kind: "ok", msg: isPublish ? "published" : "sent" });
    } catch (e) {
      setStatus({ kind: "error", msg: e instanceof Error ? e.message : String(e) });
    }
  }

  return (
    <div className="flex flex-col gap-2 text-xs" aria-label="json payload builder">
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-muted">send to</span>
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive; token-bound native */}
        <select
          aria-label="payload target"
          className={`${FIELD} w-64`}
          value={target}
          onChange={(e) => setTarget(e.target.value)}
        >
          {targets.map((t) => (
            <option key={t.tool} value={t.tool}>
              {t.label}
            </option>
          ))}
        </select>
        {isPublish && (
          /* eslint-disable-next-line no-restricted-syntax -- token-bound native input */
          <input
            aria-label="payload subject"
            className={`${FIELD} w-44`}
            placeholder="subject (e.g. ui/banner)"
            value={subject}
            onChange={(e) => setSubject(e.target.value)}
          />
        )}
        <Button aria-label="send payload" size="sm" onClick={send}>
          Send
        </Button>
        {status.kind === "ok" && (
          <span className="text-muted" aria-label="payload status">
            {status.msg}
          </span>
        )}
        {status.kind === "error" && (
          <span className="text-red-400" aria-label="payload status">
            {status.msg}
          </span>
        )}
      </div>
      <CodeEditor
        value={template}
        onChange={setTemplate}
        ariaLabel="payload template"
        placeholder='{ "text": "${newTodo}" }'
        height="120px"
      />
      <p className="text-[10px] leading-4 text-muted">
        Slots: <code>{"${var}"}</code> (a variable) and <code>{"{{value}}"}</code> (a control value),
        interpolated before send. ws scope “{ws}”.
      </p>
    </div>
  );
}
