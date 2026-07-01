// The `json` control view — a validated structured-payload editor that drives a flow node port via
// `flows.inject` (flow-dashboard-binding-ux-scope, "Structured JSON in"). The author edits a JSON
// document; on send it is PARSED and (when the port declares an input schema) VALIDATED with ajv
// before any call — never a fake accept. The parsed value becomes the node's `payload` for the next
// run. The control holds no token; the host re-checks the cap + workspace + grant on the inject.
//
// On mount it seeds the editor from the node's OWN retained input (`flows.node_state`), so it shows
// true current state after a reload/restart, not an empty default. State vs motion (rule 3): the send
// sets the retained input the next run reads — it does NOT imply the value reached a device.

import { useEffect, useMemo, useState } from "react";
import Ajv from "ajv";

import { WidgetHeader, WidgetMessage } from "../widgets/chrome";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { makeWidgetBridge } from "../builder/widgetBridge";
import { useFlowNodeValue } from "./useFlowNodeValue";
import { flowBindingOfAction } from "./flowBinding";
import { interpolateArgs, emptyScope } from "@/lib/vars";
import type { Action } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";

interface Props {
  action?: Action;
  tools: string[];
  options?: Record<string, unknown>;
  label?: string;
  scope?: VarScope;
  refreshKey?: number;
}

type Status = { kind: "idle" | "ok" | "error"; msg?: string };

export function JsonControl({
  action,
  tools,
  options,
  label,
  scope = emptyScope(),
  refreshKey = 0,
}: Props) {
  const toolsKey = tools.join("|");
  // eslint-disable-next-line react-hooks/exhaustive-deps -- re-create the bridge only when the tool SET changes
  const bridge = useMemo(() => makeWidgetBridge(tools), [toolsKey]);
  const flow = flowBindingOfAction(action);
  const seeded = useFlowNodeValue(flow?.flowId, flow?.node, flow?.port ?? "payload", "input", refreshKey);

  const [text, setText] = useState<string>("{}");
  const [status, setStatus] = useState<Status>({ kind: "idle" });

  // Seed the editor from the node's current retained input once it loads (true state, not a default).
  useEffect(() => {
    if (seeded.loading) return;
    if (seeded.value != null) setText(JSON.stringify(seeded.value, null, 2));
  }, [seeded.loading, seeded.value]);

  // The port's input schema (when the descriptor declares one) — validate against it (ajv), else free
  // JSON. Compiled once per schema; an absent/invalid schema means free JSON (no fake gate).
  const validate = useMemo(() => {
    const schema = options?.schema;
    if (!schema || typeof schema !== "object") return null;
    try {
      return new Ajv({ allErrors: true, strict: false }).compile(schema as object);
    } catch {
      return null;
    }
  }, [options?.schema]);

  async function send() {
    if (!action?.tool) return;
    setStatus({ kind: "idle" });
    let parsed: unknown;
    try {
      parsed = JSON.parse(text);
    } catch (e) {
      setStatus({ kind: "error", msg: `invalid JSON: ${e instanceof Error ? e.message : e}` });
      return; // REJECT before any call — no fake accept.
    }
    if (validate && !validate(parsed)) {
      const msg = (validate.errors ?? [])
        .map((er) => `${er.instancePath || "/"} ${er.message}`)
        .join("; ");
      setStatus({ kind: "error", msg: `schema: ${msg || "invalid"}` });
      return;
    }
    try {
      await bridge.call(
        action.tool,
        interpolateArgs(action.argsTemplate ?? {}, scope, parsed) as Record<string, unknown>,
      );
      setStatus({ kind: "ok", msg: "set" });
    } catch (e) {
      setStatus({ kind: "error", msg: e instanceof Error ? e.message : String(e) });
    }
  }

  return (
    <div className="flex h-full flex-col" aria-label={`json ${action?.tool ?? ""}`}>
      <WidgetHeader label={label ?? action?.tool ?? "json"} />
      <div className="flex flex-1 flex-col gap-2 p-2">
        <Textarea
          aria-label="json editor"
          value={text}
          spellCheck={false}
          onChange={(e) => setText(e.target.value)}
          className="min-h-0 flex-1 resize-none font-mono text-xs"
        />
        <Button size="sm" onClick={send} aria-label="send json">
          Set value
        </Button>
      </div>
      {status.kind === "error" && <WidgetMessage tone="denied">{status.msg}</WidgetMessage>}
    </div>
  );
}
