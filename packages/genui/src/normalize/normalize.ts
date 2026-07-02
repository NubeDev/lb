// `normalize` — the LLM-sloppiness pass (genui-scope "src/normalize/"). Runs during preview and at
// accept; NEVER at view time (the render path is parser-free and normalize-free — a viewer sees only
// specs that already passed accept). It fixes what it safely can and RECORDS a warning for each fix,
// so the author sees "dropped dangling child s3" / "unknown component Foo → placeholder" in the preview
// BEFORE saving — sloppiness fails at the author, not the viewer. What it cannot make sensible it leaves
// for `validate` to reject at accept. Pure: returns a new spec + findings, never throws mid-stream.
//
// The three documented fixes:
//   - unknown component        → replaced with a labelled `placeholder` component + warning
//   - dangling child id        → dropped from the parent's `children` + warning
//   - wrong-typed / missing     → coerced to the PropSpec type (or defaulted) + warning
//     required prop

import type { Catalog, PropSpec } from "../catalog/defineCatalog";
import type { Component, Finding, IrSpec, PropValue } from "../ir/types";
import { isBinding } from "../ir/types";

/** The synthetic component an unknown name is rewritten to. The catalog SHOULD define a `placeholder`
 *  entry; if it doesn't, the surface renders the inert `gu-unknown` fallback — either way, no throw. */
export const PLACEHOLDER = "placeholder";

export interface NormalizeResult {
  spec: IrSpec;
  findings: Finding[];
}

function coerce(value: PropValue, spec: PropSpec): PropValue | undefined {
  // A binding is left intact for any type — its runtime value is resolved later, not now.
  if (isBinding(value)) return value;
  switch (spec.type) {
    case "number": {
      const n = typeof value === "number" ? value : Number(value);
      return Number.isFinite(n) ? n : (spec.default as PropValue | undefined);
    }
    case "string":
      return typeof value === "string" ? value : value == null ? spec.default as PropValue : String(value);
    case "boolean":
      return typeof value === "boolean" ? value : Boolean(value);
    case "enum":
      return typeof value === "string" && spec.values?.includes(value)
        ? value
        : (spec.default as PropValue | undefined);
    case "array":
      return Array.isArray(value) ? value : (spec.default as PropValue | undefined);
    case "object":
      return value && typeof value === "object" && !Array.isArray(value) ? value : (spec.default as PropValue | undefined);
    case "binding":
      return value; // a literal where a binding was expected is allowed (static value).
    default:
      return value;
  }
}

export function normalize(spec: IrSpec, catalog: Catalog): NormalizeResult {
  const findings: Finding[] = [];
  const ids = new Set(Object.keys(spec.components));
  const out: Record<string, Component> = {};

  for (const [id, comp] of Object.entries(spec.components)) {
    const entry = catalog.resolve(comp.component);
    let component = comp.component;
    let props = { ...(comp.props ?? {}) };

    if (!entry) {
      findings.push({
        level: "warning",
        code: "unknown-component",
        message: `unknown component "${comp.component}" → placeholder`,
        componentId: id,
      });
      component = PLACEHOLDER;
      props = { label: `unknown: ${comp.component}` };
    } else {
      // Coerce known props; default missing required props (with a warning).
      for (const [key, ps] of Object.entries(entry.props)) {
        if (key in props) {
          const c = coerce(props[key], ps);
          if (c === undefined) {
            findings.push({
              level: "warning",
              code: "bad-prop",
              message: `prop "${key}" of "${comp.component}" had wrong type; dropped`,
              componentId: id,
            });
            delete props[key];
          } else if (c !== props[key]) {
            findings.push({
              level: "warning",
              code: "coerced-prop",
              message: `prop "${key}" of "${comp.component}" coerced to ${ps.type}`,
              componentId: id,
            });
            props[key] = c;
          }
        } else if (ps.required) {
          if (ps.default !== undefined) {
            props[key] = ps.default as PropValue;
            findings.push({
              level: "warning",
              code: "defaulted-prop",
              message: `required prop "${key}" of "${comp.component}" missing → default`,
              componentId: id,
            });
          }
          // No default and required-missing: leave it; validate/render handle the absence gracefully.
        }
      }
    }

    // Drop dangling children (id not in the map).
    let children = comp.children;
    if (children) {
      const kept = children.filter((c) => ids.has(c));
      if (kept.length !== children.length) {
        for (const dropped of children.filter((c) => !ids.has(c))) {
          findings.push({
            level: "warning",
            code: "dangling-child",
            message: `dropped dangling child "${dropped}" of "${id}"`,
            componentId: id,
          });
        }
      }
      children = kept;
    }

    out[id] = { id, component, props, ...(children ? { children } : {}) };
  }

  return { spec: { ...spec, components: out }, findings };
}
