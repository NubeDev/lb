// THE control renderer (editor-parity scope, step 2) — given an `OptionControl` + a value + onChange,
// renders the right shadcn control (or a typed sub-editor for the rich kinds). This is the single place
// a control kind maps to UI, so the Field tab, per-viz tabs, AND the override property editor all render
// a property IDENTICALLY (goal 4: "each property renders its normal typed control inline"). A new
// control kind is added in `types.ts` + here, once. One responsibility: control kind → rendered control.

import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Combobox } from "@/components/ui/combobox";
import type {
  DataLink,
  FieldColor,
  ThresholdsConfig,
  ValueMapping,
} from "@/lib/dashboard";
import type { OptionControl } from "./types";
import { UnitPicker } from "./controls/UnitPicker";
import { MappingsEditor } from "./controls/MappingsEditor";
import { ColorSchemeEditor } from "./controls/ColorSchemeEditor";
import { DataLinksEditor } from "./controls/DataLinksEditor";
import { ColorSwatchPicker } from "@/components/ui/color-swatch";
import { COLOR_SWATCHES } from "./palette";
import { FieldNamePicker } from "../fields/FieldNamePicker";
import { ThresholdsEditor } from "../tabs/ThresholdsEditor";

interface Props {
  control: OptionControl;
  /** The accessible name for the control (the option label). */
  label: string;
  value: unknown;
  /** Set the value; `undefined` clears the option back to its default/absent. */
  onChange: (value: unknown) => void;
}

/** Render one option's control. The value is whatever the option stores; each kind narrows it. */
export function Control({ control, label, value, onChange }: Props) {
  switch (control.kind) {
    case "number":
      return (
        <Input
          type="number"
          aria-label={label}
          className="h-8 text-xs"
          min={control.min}
          max={control.max}
          step={control.step}
          placeholder={control.placeholder}
          value={typeof value === "number" ? value : ""}
          onChange={(e) => onChange(e.target.value === "" ? undefined : Number(e.target.value))}
        />
      );
    case "text":
      return (
        <Input
          aria-label={label}
          className="h-8 text-xs"
          placeholder={control.placeholder}
          value={typeof value === "string" ? value : ""}
          onChange={(e) => onChange(e.target.value || undefined)}
        />
      );
    case "toggle":
      return <Checkbox aria-label={label} checked={value === true} onChange={(e) => onChange(e.target.checked)} />;
    case "select":
    case "multi-select":
      return (
        <Combobox
          aria-label={label}
          options={control.choices.map((c) => ({ value: c.value, label: c.label, description: "description" in c ? c.description : undefined }))}
          value={typeof value === "string" ? value : ""}
          onChange={(v) => onChange(v)}
        />
      );
    case "color":
      return (
        <ColorSwatchPicker
          aria-label={label}
          palette={COLOR_SWATCHES}
          value={typeof value === "string" ? value : ""}
          onChange={(v) => onChange(v || undefined)}
        />
      );
    case "unit":
      return <UnitPicker aria-label={label} value={typeof value === "string" ? value : ""} onChange={(v) => onChange(v || undefined)} />;
    case "field-name":
      return <FieldNamePicker aria-label={label} value={typeof value === "string" ? value : ""} onChange={(v) => onChange(v || undefined)} />;
    case "thresholds":
      return <ThresholdsEditor value={value as ThresholdsConfig | undefined} onChange={(v) => onChange(v)} />;
    case "mappings":
      return <MappingsEditor value={value as ValueMapping[] | undefined} onChange={(v) => onChange(v)} />;
    case "color-scheme":
      return <ColorSchemeEditor value={value as FieldColor | undefined} onChange={(v) => onChange(v)} />;
    case "data-links":
      return <DataLinksEditor value={value as DataLink[] | undefined} onChange={(v) => onChange(v)} />;
  }
}
