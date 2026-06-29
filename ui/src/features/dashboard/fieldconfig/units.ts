// The Grafana-unit-id → (dimension, from_unit) mapping table + the unit-picker list (viz field-config
// scope, "The unit picker / mapping table"). This is the SINGLE source of truth that turns a Grafana
// `fieldConfig.unit` id into our prefs closed-dimension enum (the `from_unit` for `format.quantity`),
// or marks it a PASSTHROUGH (currency / arbitrary suffix → `format.number` + a literal suffix).
//
// One responsibility: the table + lookups. The actual formatting (calling the prefs bridge / the
// fallback) lives in `format.ts` — never here, never in a renderer.

/** Our prefs closed-dimension enum (a subset for Phase 1; grows with `lb-prefs`). */
export type Dimension =
  | "temperature"
  | "wind_speed"
  | "speed"
  | "distance"
  | "mass"
  | "pressure"
  | "data"
  | "percent"
  | "time";

/** How a Grafana unit id renders. `quantity` → a dimension+from_unit for `format.quantity` (unit
 *  conversion + locale); `number` → `format.number` (no conversion) with an optional static suffix;
 *  `none` → a bare localized number; `datetime` → `format.datetime` with a pattern. */
export type UnitKind = "quantity" | "number" | "none" | "datetime";

/** A resolved unit mapping for one Grafana unit id. */
export interface UnitMapping {
  /** The Grafana unit id (the key). */
  id: string;
  /** A short human label for the picker + the fallback suffix. */
  label: string;
  kind: UnitKind;
  /** `quantity`: the canonical dimension + source unit. */
  dimension?: Dimension;
  fromUnit?: string;
  /** `number`/`none`/`datetime`: the static suffix or date pattern to append/use. */
  suffix?: string;
  pattern?: string;
}

/** The closed table — Grafana id → mapping. Extended as `lb-prefs` grows its dimension enum. A
 *  Grafana id NOT in this table degrades to a passthrough number + the raw id as suffix (never wrong). */
const TABLE: Record<string, UnitMapping> = {
  // dimensionful — convert + localize via format.quantity
  celsius: { id: "celsius", label: "°C", kind: "quantity", dimension: "temperature", fromUnit: "degree_celsius" },
  fahrenheit: { id: "fahrenheit", label: "°F", kind: "quantity", dimension: "temperature", fromUnit: "degree_fahrenheit" },
  kelvin: { id: "kelvin", label: "K", kind: "quantity", dimension: "temperature", fromUnit: "kelvin" },
  velocitykmh: { id: "velocitykmh", label: "km/h", kind: "quantity", dimension: "wind_speed", fromUnit: "kilometer_per_hour" },
  velocityms: { id: "velocityms", label: "m/s", kind: "quantity", dimension: "wind_speed", fromUnit: "meter_per_second" },
  velocitymph: { id: "velocitymph", label: "mph", kind: "quantity", dimension: "speed", fromUnit: "mile_per_hour" },
  velocityknot: { id: "velocityknot", label: "kn", kind: "quantity", dimension: "wind_speed", fromUnit: "knot" },
  lengthm: { id: "lengthm", label: "m", kind: "quantity", dimension: "distance", fromUnit: "meter" },
  lengthkm: { id: "lengthkm", label: "km", kind: "quantity", dimension: "distance", fromUnit: "kilometer" },
  bytes: { id: "bytes", label: "B", kind: "quantity", dimension: "data", fromUnit: "byte" },
  decbytes: { id: "decbytes", label: "B", kind: "quantity", dimension: "data", fromUnit: "byte" },
  pressurehpa: { id: "pressurehpa", label: "hPa", kind: "quantity", dimension: "pressure", fromUnit: "hectopascal" },
  kg: { id: "kg", label: "kg", kind: "quantity", dimension: "mass", fromUnit: "kilogram" },
  // percent — localized number + literal sign (no dimension conversion)
  percent: { id: "percent", label: "%", kind: "number", suffix: "%" },
  percentunit: { id: "percentunit", label: "%", kind: "number", suffix: "%" },
  // plain numbers
  short: { id: "short", label: "", kind: "none" },
  none: { id: "none", label: "", kind: "none" },
  // currency — passthrough display unit
  currencyUSD: { id: "currencyUSD", label: "$", kind: "number", suffix: " USD" },
  currencyEUR: { id: "currencyEUR", label: "€", kind: "number", suffix: " EUR" },
  // datetime patterns
  dateTimeAsIso: { id: "dateTimeAsIso", label: "ISO", kind: "datetime", pattern: "iso" },
  "time:YYYY-MM-DD": { id: "time:YYYY-MM-DD", label: "date", kind: "datetime", pattern: "YYYY-MM-DD" },
};

/** The picker list — every mapped unit, grouped-friendly order (the field-tab unit dropdown source). */
export function unitOptions(): UnitMapping[] {
  return Object.values(TABLE);
}

/** Resolve a Grafana unit id to its mapping. An unknown id degrades to a PASSTHROUGH number whose
 *  suffix is the raw id (honest, never silently cross-dimension wrong — field-config scope, Risks).
 *  A `custom:<suffix>` id is a passthrough with that literal suffix. */
export function resolveUnit(unitId: string | undefined): UnitMapping {
  if (!unitId) return { id: "", label: "", kind: "none" };
  const known = TABLE[unitId];
  if (known) return known;
  if (unitId.startsWith("custom:")) {
    const suffix = unitId.slice("custom:".length);
    return { id: unitId, label: suffix, kind: "number", suffix: ` ${suffix}` };
  }
  if (unitId.startsWith("time:")) {
    return { id: unitId, label: "date", kind: "datetime", pattern: unitId.slice("time:".length) };
  }
  // Unmapped: passthrough number + the raw id as a visible suffix (degrade, never throw or mis-convert).
  return { id: unitId, label: unitId, kind: "number", suffix: ` ${unitId}` };
}
