// `validate` — pure structural check of an `IrSpec` against a catalog. Render-stratum, so it takes the
// catalog as an argument (never imports a specific one). Returns `Finding[]`; `errors()` filters the
// blocking ones. Used three ways: at authoring accept (loud), by the host on save (name resolution),
// and as view-time defense-in-depth. It does NOT mutate — normalize is the fixer; validate only reports.

import type { Catalog } from "../catalog/defineCatalog";
import type { Finding, IrSpec } from "./types";
import { IR_VERSION } from "./types";

export interface ValidateOptions {
  catalog: Catalog;
}

export function validate(spec: IrSpec, opts: ValidateOptions): Finding[] {
  const findings: Finding[] = [];
  const { catalog } = opts;

  if (typeof spec.v !== "number") {
    findings.push({ level: "error", code: "missing-version", message: "IR spec has no numeric `v`" });
  } else if (spec.v > IR_VERSION) {
    findings.push({
      level: "error",
      code: "future-version",
      message: `IR spec v${spec.v} is newer than this renderer (v${IR_VERSION})`,
    });
  }

  const ids = new Set(Object.keys(spec.components));
  if (!spec.surface || typeof spec.surface.root !== "string" || spec.surface.root === "") {
    findings.push({ level: "error", code: "no-root", message: "surface has no root component" });
  } else if (!ids.has(spec.surface.root)) {
    findings.push({
      level: "error",
      code: "dangling-root",
      message: `surface root "${spec.surface.root}" is not a defined component`,
    });
  }

  for (const [id, comp] of Object.entries(spec.components)) {
    if (comp.id !== id) {
      findings.push({ level: "error", code: "id-mismatch", message: `component key "${id}" != id "${comp.id}"`, componentId: id });
    }
    if (!catalog.has(comp.component)) {
      findings.push({
        level: "error",
        code: "unknown-component",
        message: `component "${comp.component}" (id ${id}) is not in the catalog`,
        componentId: id,
      });
    }
    for (const child of comp.children ?? []) {
      if (!ids.has(child)) {
        findings.push({
          level: "warning",
          code: "dangling-child",
          message: `child "${child}" of "${id}" is not a defined component`,
          componentId: id,
        });
      }
    }
  }
  return findings;
}

export function errors(findings: Finding[]): Finding[] {
  return findings.filter((f) => f.level === "error");
}

export function warnings(findings: Finding[]): Finding[] {
  return findings.filter((f) => f.level === "warning");
}
