// The STANDARD field options (editor-parity scope, step 2) — Grafana's `standardFieldConfig` set:
// universal per-field options (display name, unit, decimals, min/max, no-value, thresholds, color
// scheme, value mappings, data links) that apply to every viz and back every override property. Each
// entry is one `OptionDef`; the Field tab renders them by group, the override picker offers them, and
// options search indexes them — all from here. `scope: "fieldConfig"` → they write
// `fieldConfig.defaults.<id>` (and are exactly what an override sets per-field). One responsibility:
// the standard field option catalog.

import type { OptionDef } from "../types";

const GROUP = "Standard options";

export const STANDARD_OPTIONS: OptionDef[] = [
  {
    id: "displayName",
    label: "Display name",
    group: GROUP,
    scope: "fieldConfig",
    control: { kind: "text", placeholder: "override the field's name" },
    default: undefined,
    keywords: ["title", "rename", "alias"],
  },
  {
    id: "unit",
    label: "Unit",
    group: GROUP,
    scope: "fieldConfig",
    control: { kind: "unit" },
    default: undefined,
    keywords: ["celsius", "bytes", "percent", "currency", "format"],
  },
  {
    id: "decimals",
    label: "Decimals",
    group: GROUP,
    scope: "fieldConfig",
    control: { kind: "number", min: 0, max: 10, step: 1, placeholder: "auto" },
    default: undefined,
    keywords: ["precision", "rounding"],
  },
  {
    id: "min",
    label: "Min",
    group: GROUP,
    scope: "fieldConfig",
    control: { kind: "number", placeholder: "auto" },
    default: undefined,
    keywords: ["range", "scale"],
  },
  {
    id: "max",
    label: "Max",
    group: GROUP,
    scope: "fieldConfig",
    control: { kind: "number", placeholder: "auto" },
    default: undefined,
    keywords: ["range", "scale"],
  },
  {
    id: "noValue",
    label: "No value",
    group: GROUP,
    scope: "fieldConfig",
    control: { kind: "text", placeholder: "text when null/empty" },
    default: undefined,
    keywords: ["null", "empty", "placeholder"],
  },
  {
    id: "color",
    label: "Color scheme",
    group: GROUP,
    scope: "fieldConfig",
    control: { kind: "color-scheme" },
    default: undefined,
    keywords: ["palette", "gradient", "fixed", "thresholds"],
  },
  {
    id: "thresholds",
    label: "Thresholds",
    group: "Thresholds",
    scope: "fieldConfig",
    control: { kind: "thresholds" },
    default: undefined,
    keywords: ["steps", "levels", "alert", "color"],
  },
  {
    id: "mappings",
    label: "Value mappings",
    group: "Value mappings",
    scope: "fieldConfig",
    control: { kind: "mappings" },
    default: undefined,
    keywords: ["map", "replace", "label", "text", "special", "range"],
  },
  {
    id: "links",
    label: "Data links",
    group: "Data links",
    scope: "fieldConfig",
    control: { kind: "data-links" },
    default: undefined,
    keywords: ["url", "drilldown", "link"],
  },
];
