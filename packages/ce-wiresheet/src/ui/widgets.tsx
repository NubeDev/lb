// Core widgets for `layout` views: text, value (live), button. Each self-
// registers on import. `value` binds to the bound component's prop and streams
// from the value store (per-widget subscription, like FunctionBlock's ValueRow).

import { memo } from "react";
import { useStructural, useValues, propertyDataType } from "../lib/store";
import { facetFor, FACET_PROP } from "../lib/facet";
import { fmtValueFacet, inferDataType } from "../lib/format";
import { registerWidget, type WidgetProps } from "./registry";
import type { FlexValue } from "../lib/engine-types";

function TextWidget({ node }: WidgetProps) {
  return <div style={{ fontSize: 12, color: "hsl(var(--muted-foreground))", padding: "2px 0" }}>{node.text}</div>;
}

/** Live value of `ctx.componentUid`'s prop `node.bind.prop`. */
const ValueWidget = memo(function ValueWidget({ node, ctx }: WidgetProps) {
  const propName = node.bind?.prop;
  const comp = useStructural((s) =>
    ctx.componentUid != null ? s.components.get(ctx.componentUid) : undefined,
  );
  const uid = comp && propName ? comp.properties[propName]?.uid : undefined;
  const v = useValues((s) => (uid != null ? s.values.get(uid) : undefined));

  let text = "—";
  if (uid != null) {
    const facet = facetFor(comp!.uid, comp!.properties[FACET_PROP]?.value as string | undefined);
    text = fmtValueFacet(v, propertyDataType.get(uid) ?? inferDataType(v), facet.get(uid));
  } else if (propName && ctx.componentUid == null) {
    text = "(no selection)";
  }

  return (
    <div style={{ display: "flex", justifyContent: "space-between", gap: 12, padding: "3px 0", fontSize: 12 }}>
      <span style={{ color: "hsl(var(--muted-foreground))" }}>{node.label ?? propName}</span>
      <span style={{ color: "hsl(var(--foreground))", fontVariantNumeric: "tabular-nums" }}>{text}</span>
    </div>
  );
});

function ButtonWidget({ node, ctx }: WidgetProps) {
  const a = node.action;
  return (
    <button
      onClick={() => {
        if (!a) return;
        if (a.confirm && !window.confirm(a.confirm)) return;
        if (a.target === "component" && ctx.componentUid != null) {
          void ctx.callAction?.(ctx.componentUid, a.name, a.params as Record<string, FlexValue> | undefined);
        } else {
          // eslint-disable-next-line no-console
          console.info("[ui action]", a.name, "target", a.target);
        }
      }}
      style={{
        marginTop: 6,
        padding: "5px 12px",
        fontSize: 12,
        color: "hsl(var(--foreground))",
        background: "hsl(var(--border))",
        border: "1px solid hsl(var(--input))",
        borderRadius: 4,
        cursor: "pointer",
      }}
    >
      {node.label ?? a?.label ?? "Action"}
    </button>
  );
}

registerWidget("text", TextWidget);
registerWidget("value", ValueWidget);
registerWidget("button", ButtonWidget);
