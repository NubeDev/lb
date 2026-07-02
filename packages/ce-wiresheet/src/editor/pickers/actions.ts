import type { FlexValue } from "../../lib/engine-types";

// Action spec for a component type, sourced from `/schema`. The signature
// (params/returns) is static per type, so it rides the cached schema — opening
// the picker is a pure in-memory lookup, no per-right-click fetch. Future
// per-instance availability layers on top via `getActionsFor` without changing
// this shape (see the action-discovery design notes).
export interface ActionParamDef {
  name: string;
  type: string; // FlexValue type: bool | int | i32 | i64 | f32 | f64 | str | ...
  default?: FlexValue;
  label?: string;
  required?: boolean;
  enum?: FlexValue[];
}
export interface ActionReturnDef {
  name: string;
  type: string;
}
export interface ActionDef {
  name: string;
  label?: string;
  description?: string;
  params?: ActionParamDef[];
  returns?: ActionReturnDef[];
}

// Map a FlexValue type tag onto an input kind. Numeric tags cover the engine's
// int/float family; everything non-bool/non-numeric renders as text.
export function actionKind(type: string): "bool" | "num" | "str" {
  const t = type.toLowerCase();
  if (t === "bool" || t === "boolean") return "bool";
  if (/^(u?int\d*|[iuf]\d+|float|double|number)$/.test(t)) return "num";
  return "str";
}
export function defaultForType(type: string): FlexValue {
  const k = actionKind(type);
  return k === "bool" ? false : k === "num" ? 0 : "";
}
export function coerceParam(type: string, raw: string): FlexValue {
  const k = actionKind(type);
  if (k === "num") {
    const n = Number(raw);
    return Number.isFinite(n) ? n : 0;
  }
  if (k === "bool") return raw === "true" || raw === "1";
  return raw;
}
