// Shared value formatting for the wiresheet nodes and the table — so a value
// renders identically in both. Alias label wins; otherwise apply the facet's
// decimals (clamped to toFixed's 0–100 range) + unit, falling back to base
// type-aware formatting.

import {
  DATATYPE_BOOL,
  DATATYPE_NUMBER,
  DATATYPE_STRING,
  type PropertyDataType,
} from "./engine-types";
import type { DecodedValue } from "./wire";
import { aliasLabel, MAX_DECIMALS, type PropFacet } from "./facet";

// CONFIG / NULL props carry no schema dataType — infer from the runtime value.
export function inferDataType(v: unknown): PropertyDataType {
  if (typeof v === "boolean") return DATATYPE_BOOL;
  if (typeof v === "string") return DATATYPE_STRING;
  return DATATYPE_NUMBER;
}

export function fmtValue(v: DecodedValue | undefined, dt: PropertyDataType): string {
  if (v === undefined) return "—";
  if (typeof v === "bigint") return v.toString();
  if (typeof v === "boolean") return v ? "true" : "false";
  if (typeof v === "string") return JSON.stringify(v).slice(1, -1);
  // number
  if (dt === DATATYPE_BOOL) return v ? "true" : "false";
  if (Number.isInteger(v)) return v.toString();
  return v.toFixed(2);
}

// Render an epoch number as a LOCAL date/time. Auto-detects seconds vs millis
// (CE int epochs are seconds; < 1e12 → seconds). Date renders in the browser's
// local timezone, which is the "local version" of the UTC-stored value.
export function fmtDateTime(v: number, format: "datetime" | "date" | "time"): string {
  if (!Number.isFinite(v) || v === 0) return "—";
  const d = new Date(Math.abs(v) < 1e12 ? v * 1000 : v);
  if (Number.isNaN(d.getTime())) return String(v);
  if (format === "date") return d.toLocaleDateString();
  if (format === "time") return d.toLocaleTimeString();
  return d.toLocaleString();
}

export function fmtValueFacet(
  v: DecodedValue | undefined,
  dt: PropertyDataType,
  facet: PropFacet | undefined,
): string {
  const al = aliasLabel(facet?.aliases, v);
  if (al != null) return al;
  if (facet?.format && typeof v === "number") return fmtDateTime(v, facet.format);
  let base: string;
  if (facet?.decimals != null && Number.isFinite(facet.decimals) && typeof v === "number") {
    base = v.toFixed(Math.min(MAX_DECIMALS, Math.max(0, Math.trunc(facet.decimals))));
  } else {
    base = fmtValue(v, dt);
  }
  return facet?.unit && base !== "—" ? `${base} ${facet.unit}` : base;
}
